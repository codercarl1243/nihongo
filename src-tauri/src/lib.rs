use std::path::PathBuf;
use std::sync::Mutex;

use audio_engine::{AudioManager, AudioPlayer, EngineConfig};
use db::Db;
use llm::{ChatMessage, SidecarClient};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_stream::StreamExt;
use tutor::{parse_response_pub, parse_summary_pub, TutorSession};

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct AppState {
    audio:   Mutex<AudioManager>,
    player:  Mutex<AudioPlayer>,
    llm:     SidecarClient,
    db:      Mutex<Db>,
    session: Mutex<Option<TutorSession>>,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
struct TranscriptEvent    { text: String }
#[derive(Clone, Serialize)]
struct ResponseTokenEvent { token: String }
#[derive(Clone, Serialize)]
struct ResponseDoneEvent  { full_response: String, milestone: bool }
#[derive(Clone, Serialize)]
struct ErrorEvent         { message: String }

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn start_session(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let streams = state.audio
        .lock().unwrap()
        .start(EngineConfig::default())
        .map_err(|e| e.to_string())?;

    state.player.lock().unwrap().start().map_err(|e| e.to_string())?;

    {
        let db = state.db.lock().unwrap();
        let session = TutorSession::new(&db).map_err(|e| e.to_string())?;
        *state.session.lock().unwrap() = Some(session);
    }

    let llm = state.llm.clone();
    let mut turn_stream = streams.turn_end;

    tokio::spawn(async move {
        while let Some(audio_chunk) = turn_stream.next().await {
            let app2 = app.clone();
            let llm2 = llm.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_turn(audio_chunk, &app2, &llm2).await {
                    let _ = app2.emit("error", ErrorEvent { message: e.to_string() });
                }
            });
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_session(state: State<'_, AppState>) -> Result<(), String> {
    state.audio.lock().unwrap().stop().map_err(|e| e.to_string())?;
    state.player.lock().unwrap().stop();
    if let Some(s) = state.session.lock().unwrap().take() {
        state.db.lock().unwrap()
            .end_session(s.session_id())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn barge_in(state: State<'_, AppState>) {
    state.player.lock().unwrap().stop();
}

// ---------------------------------------------------------------------------
// Per-turn pipeline
// All locks are acquired, used, and DROPPED before any .await.
// ---------------------------------------------------------------------------

async fn handle_turn(
    audio: Vec<f32>,
    app: &AppHandle,
    llm: &SidecarClient,
) -> anyhow::Result<()> {
    let state = app.state::<AppState>();

    // ── 1. ASR ──────────────────────────────────────────────────────────────
    let transcript = llm.transcribe(&audio).await?;
    if transcript.trim().is_empty() {
        return Ok(());
    }
    app.emit("transcript", TranscriptEvent { text: transcript.clone() })?;

    // ── 2. Append user turn, snapshot history ────────────────────────────────
    // Lock acquired and dropped before any await.
    let messages: Vec<ChatMessage> = {
        let mut sg = state.session.lock().unwrap();
        let s = sg.as_mut().ok_or_else(|| anyhow::anyhow!("no active session"))?;
        s.push_user(&transcript);
        s.current_messages().to_vec()
    };

    // ── 3. Stream LLM tokens ────────────────────────────────────────────────
    let mut full_text = String::new();
    {
        let mut stream = llm.chat_stream(&messages).await?;
        while let Some(chunk) = stream.next().await {
            let token = chunk?;
            if !token.is_empty() {
                full_text.push_str(&token);
                app.emit("response_token", ResponseTokenEvent { token })?;
            }
        }
    }

    let tutor_resp = parse_response_pub(&full_text);
    app.emit("response_done", ResponseDoneEvent {
        full_response: tutor_resp.response.clone(),
        milestone: tutor_resp.milestone,
    })?;

    // ── 4. Persist turn (sync, lock dropped before await) ───────────────────
    let needs_compact: bool = {
        let mut sg = state.session.lock().unwrap();
        let s = sg.as_mut().ok_or_else(|| anyhow::anyhow!("no active session"))?;
        let db = state.db.lock().unwrap();
        s.record_turn_sync(&transcript, &tutor_resp, &db)?
    };

    // ── 5. Context compaction (if milestone + threshold reached) ────────────
    if needs_compact {
        compact_context(app, llm).await?;
    }

    // ── 6. TTS playback ─────────────────────────────────────────────────────
    if !tutor_resp.response.is_empty() {
        state.player.lock().unwrap().resume();
        let wav = llm.speak(&tutor_resp.response).await?;
        let pcm = wav_to_f32(&wav)?;
        state.player.lock().unwrap().play_chunk(&pcm)?;
    }

    Ok(())
}

/// Build a summary, save it, then reset the session context.
/// Called after a milestone when the context threshold is crossed.
async fn compact_context(app: &AppHandle, llm: &SidecarClient) -> anyhow::Result<()> {
    let state = app.state::<AppState>();

    // Snapshot the summary-request messages (lock dropped before await)
    let summary_msgs: Vec<ChatMessage> = {
        let sg = state.session.lock().unwrap();
        let s = sg.as_ref().ok_or_else(|| anyhow::anyhow!("no active session"))?;
        s.summary_request_messages()
    };

    // Stream the summary (no locks held)
    let mut raw_summary = String::new();
    {
        let mut stream = llm.chat_stream(&summary_msgs).await?;
        while let Some(chunk) = stream.next().await {
            raw_summary.push_str(&chunk?);
        }
    }

    let summary = parse_summary_pub(&raw_summary);

    // Save summary and build new system prompt (sync, locks dropped before next await)
    let new_system: ChatMessage = {
        let db = state.db.lock().unwrap();
        db.save_lesson_summary(&summary)?;
        let profile = db.learner_profile()?;
        let saved_summary = db.latest_lesson_summary()?;
        tutor::build_system_prompt_pub(&profile, saved_summary.as_ref())
    };

    // Reset the session context
    {
        let mut sg = state.session.lock().unwrap();
        if let Some(s) = sg.as_mut() {
            s.reset_context(new_system);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// WAV → f32 (PCM_16 LE, standard 44-byte header)
// ---------------------------------------------------------------------------

fn wav_to_f32(wav: &[u8]) -> anyhow::Result<Vec<f32>> {
    if wav.len() < 44 {
        anyhow::bail!("WAV payload too short");
    }
    let samples = wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect();
    Ok(samples)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Db::open(&db_path()).expect("failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            audio:   Mutex::new(AudioManager::new()),
            player:  Mutex::new(AudioPlayer::new()),
            llm:     SidecarClient::new(),
            db:      Mutex::new(db),
            session: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            start_session,
            stop_session,
            barge_in,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nihongo")
        .join("nihongo.db")
}
