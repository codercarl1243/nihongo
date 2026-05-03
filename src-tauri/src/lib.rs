use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use audio_engine::{AudioManager, AudioPlayer, EngineConfig};
use chrono::Timelike as _;
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

// Compile-time path to the sidecar directory; resolves relative to src-tauri/.
const SIDECAR_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sidecar");

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
struct SidecarStatusEvent { state: String, message: Option<String> }
#[derive(Clone, Serialize)]
struct TranscriptEvent    { text: String }
#[derive(Clone, Serialize)]
struct ResponseDoneEvent  { full_response: String, milestone: bool }
#[derive(Clone, Serialize)]
struct SessionReadyEvent  { greeting: String }
#[derive(Clone, Serialize)]
struct ErrorEvent         { message: String }

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn start_session(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if !state.llm.is_ready().await {
        return Err(
            "Sidecar is still loading — please wait for models to initialise.".to_string()
        );
    }

    // Silence threshold scales with proficiency: beginners need more time to
    // retrieve words; near-native speakers can pace like natural conversation.
    let silence_ms = {
        let db = state.db.lock().unwrap();
        match db.session_context().map(|c| c.profile.current_level).unwrap_or(5) {
            1 | 2 => 700,  // N1/N2 — near-native pacing
            3     => 900,  // N3 — intermediate
            _     => 1200, // N4/N5 — beginner
        }
    };

    let streams = state.audio
        .lock().unwrap()
        .start(EngineConfig {
            silence_threshold: Duration::from_millis(silence_ms),
            ..EngineConfig::default()
        })
        .map_err(|e| e.to_string())?;

    state.player.lock().unwrap().start().map_err(|e| e.to_string())?;

    {
        let db = state.db.lock().unwrap();
        let session = TutorSession::new(&db).map_err(|e| e.to_string())?;
        *state.session.lock().unwrap() = Some(session);
    }

    let llm = state.llm.clone();
    let mut turn_stream = streams.turn_end;
    let mut partial_stream = streams.partial;
    let greeting_app = app.clone();

    // Keep the partial receiver alive; drain without consuming.
    // Replace this task with speculative ASR when that feature is added.
    tokio::spawn(async move {
        while partial_stream.next().await.is_some() {}
    });

    tokio::spawn(async move {
        while let Some(audio_chunk) = turn_stream.next().await {
            if let Err(e) = handle_turn(audio_chunk, &app, &llm).await {
                let _ = app.emit("error", ErrorEvent { message: e.to_string() });
            }
        }
    });

    tokio::spawn(async move {
        if let Err(e) = send_greeting(greeting_app.clone()).await {
            let _ = greeting_app.emit("error", ErrorEvent { message: e.to_string() });
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
// Greeting / warm-up
// ---------------------------------------------------------------------------

// TODO: utilize username from learner profile instead of hardcoded name
fn build_greeting() -> &'static str {
    let hour = chrono::Local::now().hour();
    match hour {
        5..=11  => "Good morning, Carl!",
        12..=16 => "Good afternoon, Carl!",
        17..=20 => "Good evening, Carl!",
        _       => "Good evening, Carl!",
    }
}

/// Speaks a hardcoded greeting via TTS directly — no LLM involved.
/// Emits `session_ready` once audio has been queued for playback.
async fn send_greeting(app: AppHandle) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    let greeting = build_greeting();

    state.audio.lock().unwrap().mute();
    state.player.lock().unwrap().resume();
    let wav = state.llm.speak(greeting).await?;
    let pcm = wav_to_f32(&wav)?;
    state.player.lock().unwrap().play_chunk(&pcm, 24_000)?;
    let duration = Duration::from_secs_f64(pcm.len() as f64 / 24_000.0);
    wait_for_playback(&state, duration).await;

    app.emit("response_done", ResponseDoneEvent {
        full_response: greeting.to_string(),
        milestone: false,
    })?;
    app.emit("session_ready", SessionReadyEvent { greeting: greeting.to_string() })?;

    Ok(())
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
    let transcript = llm.transcribe(&normalize_peak(&audio)).await?;
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

    // ── 3+6. Pipeline: LLM stream → sentence channel → TTS → play ─────────
    // Sentences are sent over a buffered channel so TTS for sentence N can
    // begin while the LLM is still generating sentence N+1.
    let (sentence_tx, mut sentence_rx) = tokio::sync::mpsc::channel::<String>(8);

    let llm_clone  = llm.clone();
    let msgs_clone = messages.clone();
    let llm_task   = tokio::spawn(async move {
        let mut buf  = String::new();
        let mut full = String::new();
        let mut stream = llm_clone.chat_stream(&msgs_clone).await?;
        while let Some(chunk) = stream.next().await {
            let tok = chunk?;
            full.push_str(&tok);
            buf.push_str(&tok);
            while let Some(sent) = flush_sentence(&mut buf) {
                sentence_tx.send(sent).await.ok();
            }
        }
        // flush remainder (no terminal punctuation)
        let rem = buf.trim().to_string();
        if !rem.is_empty() { sentence_tx.send(rem).await.ok(); }
        Ok::<String, anyhow::Error>(full)
    });

    let mut muted         = false;
    let mut total_samples = 0usize;
    while let Some(sentence) = sentence_rx.recv().await {
        if !muted {
            state.audio.lock().unwrap().mute();
            state.player.lock().unwrap().resume();
            muted = true;
        }
        let wav = llm.speak(&sentence).await?;
        let pcm = wav_to_f32(&wav)?;
        total_samples += pcm.len();
        state.player.lock().unwrap().play_chunk(&pcm, 24_000)?;
    }

    let full_text  = llm_task.await??;
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

    // ── 4b. Vocabulary introduction — best-effort, never breaks the pipeline ──
    {
        let db = state.db.lock().unwrap();
        if let Ok(ctx) = db.session_context() {
            if let Some(ref active) = ctx.current_topic {
                if let Ok(ids) = db.find_vocab_in_text(&transcript, active.topic.id) {
                    for vid in ids {
                        let _ = db.introduce_word(vid);
                    }
                }
            }
        }
    }

    // ── 5. Context compaction (if milestone + threshold reached) ────────────
    if needs_compact {
        compact_context(app, llm).await?;
    }

    // ── Wait for all queued audio / unmute ───────────────────────────────────
    if total_samples > 0 {
        let duration = Duration::from_secs_f64(total_samples as f64 / 24_000.0);
        wait_for_playback(&state, duration).await;
    } else {
        state.audio.lock().unwrap().unmute();
    }

    Ok(())
}

/// Sleeps for `duration`, polling every 50ms for a barge-in (player stopped).
/// Unmutes capture when done or barged in.
async fn wait_for_playback(state: &AppState, duration: Duration) {
    let step        = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;

    while elapsed < duration {
        tokio::time::sleep(step).await;
        elapsed += step;
        if state.player.lock().unwrap().is_stopped() {
            break;
        }
    }

    state.audio.lock().unwrap().unmute();
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

    // Save summary, then rebuild context from fresh session_context() snapshot.
    let new_system: ChatMessage = {
        let db = state.db.lock().unwrap();
        db.save_lesson_summary(&summary)?;
        let ctx = db.session_context()?;
        tutor::build_system_prompt_pub(&ctx)
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
// Audio / text helpers
// ---------------------------------------------------------------------------

/// Extract the next complete sentence from `buf` (up to 。！？!?), consuming it.
fn flush_sentence(buf: &mut String) -> Option<String> {
    const ENDS: &[char] = &['。', '！', '？', '!', '?'];
    let pos = buf.find(|c: char| ENDS.contains(&c))?;
    let end = pos + buf[pos..].chars().next().unwrap().len_utf8();
    let sentence = buf[..end].trim().to_string();
    *buf = buf[end..].trim_start().to_string();
    if sentence.is_empty() { None } else { Some(sentence) }
}

/// Scale audio so its peak is 0.9, leaving silence untouched.
/// Compensates for low-gain microphones (e.g. earbuds).
fn normalize_peak(audio: &[f32]) -> Vec<f32> {
    let peak = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak < 0.001 { return audio.to_vec(); }
    let scale = 0.9 / peak;
    audio.iter().map(|s| (s * scale).clamp(-1.0, 1.0)).collect()
}

// ---------------------------------------------------------------------------
// Sidecar lifecycle
// ---------------------------------------------------------------------------

async fn sidecar_is_up() -> bool {
    reqwest::get("http://127.0.0.1:8091/health")
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Spawned at startup. Starts the sidecar if not already running, then polls
/// until ready. Emits `sidecar_status` events for the frontend loading state.
/// The sidecar is intentionally left running when the app exits so models
/// stay warm in GPU memory between sessions.
async fn start_sidecar_background(app: AppHandle) {
    let emit = |state: &str, message: Option<String>| {
        let _ = app.emit("sidecar_status", SidecarStatusEvent {
            state: state.to_string(),
            message,
        });
    };

    emit("warming_up", None);

    // Already running — nothing to do.
    if sidecar_is_up().await {
        emit("ready", None);
        return;
    }

    // Spawn start.sh detached. stdout/stderr inherit so log output stays
    // visible in the terminal where the app was launched.
    if let Err(e) = std::process::Command::new("bash")
        .arg("start.sh")
        .current_dir(SIDECAR_DIR)
        .spawn()
    {
        emit("error", Some(format!("Failed to launch sidecar: {e}")));
        return;
    }

    // Poll every 2 s for up to 2 minutes.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if sidecar_is_up().await {
            emit("ready", None);
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            emit("error", Some("Sidecar did not become ready within 2 minutes".into()));
            return;
        }
    }
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
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(start_sidecar_background(handle));
            Ok(())
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
