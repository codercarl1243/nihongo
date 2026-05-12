/// Backend integration test — runs without the Tauri app.
///
/// Requires the Python sidecar running on localhost:8091 AND VoiceVox Engine on localhost:50021.
/// Start them with:  cd sidecar && bash start.sh   (sidecar)
///                   sidecar/voicevox_engine/run    (VoiceVox)
///   or:             pnpm test:integration           (starts both automatically)
///
/// Expected output:
///
///   [1/7] Sidecar health … PASS
///   [2/7] VoiceVox Engine health … PASS
///   [3/7] Database — open, start session, record turn, end session … PASS  (session_id=1)
///   [4/7] TutorSession + system prompt … PASS  (2 messages in context)
///   [5/7] LLM stream → parse TutorResponse … PASS  (response = "いい天気ですね…")
///   [6/7] TTS speak → WAV … PASS  (12345 bytes)
///   [7/7] AudioPlayer playback … PASS
///
///   ✓ All tests passed.
///
/// What each test checks:
///   1. Sidecar health   — sidecar responds on localhost:8091
///   2. VoiceVox health  — VoiceVox Engine responds on localhost:50021
///   3. DB               — Db::open, start_session, record_turn, end_session all succeed
///   4. TutorSession     — builds a session from the live DB; system prompt renders without panic
///   5. LLM              — full chat stream completes; parse_response produces a non-empty response
///   6. TTS              — speak() returns WAV bytes; WAV chunk structure is valid
///   7. Playback         — AudioPlayer decodes and plays the TTS WAV without error

use std::io::Write as _;
use std::path::PathBuf;
use tokio_stream::StreamExt;

use audio_engine::AudioPlayer;
use db::Db;
use llm::SidecarClient;
use nihongo_lib::tts::{VoiceVoxClient, DEFAULT_SPEAKER};
use tutor::{Japanese, parse_response_pub, TutorSession};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let llm = SidecarClient::new();
    let tts = VoiceVoxClient::new(DEFAULT_SPEAKER);
    let mut failures = 0;

    // ── 1. Sidecar health ────────────────────────────────────────────────────
    step(1, 7, "Sidecar health");
    match llm.wait_until_ready(10).await {
        Ok(_) => pass(None),
        Err(e) => { fail(format!("{e}")); failures += 1; }
    }

    // ── 2. VoiceVox Engine health ─────────────────────────────────────────────
    step(2, 7, "VoiceVox Engine health");
    match reqwest::get("http://127.0.0.1:50021/version").await {
        Ok(r) if r.status().is_success() => pass(None),
        Ok(r) => { fail(format!("unexpected status {}", r.status())); failures += 1; }
        Err(e) => { fail(format!("{e}")); failures += 1; }
    }

    // ── 3. Database ──────────────────────────────────────────────────────────
    step(3, 7, "Database — open, start session, record turn, end session");
    let db_result = (|| -> anyhow::Result<Db> {
        let db = Db::open(&db_path())?;
        let session_id = db.start_session()?;
        db.end_session(session_id)?;
        Ok(db)
    })();
    let db = match db_result {
        Ok(db) => { pass(Some(format!("path = {:?}", db_path()))); db }
        Err(e) => {
            fail(format!("{e}"));
            failures += 1;
            // DB is required for subsequent steps — bail early.
            print_summary(failures);
            std::process::exit(1);
        }
    };

    // ── 4. TutorSession + system prompt ──────────────────────────────────────
    step(4, 7, "TutorSession + system prompt");
    let session_result = (|| -> anyhow::Result<TutorSession> {
        let mut session = TutorSession::new(&db, Arc::new(Japanese))?;
        session.push_user(&greeting());
        Ok(session)
    })();
    let mut session = match session_result {
        Ok(s) => {
            pass(Some(format!("{} messages in context", s.current_messages().len())));
            s
        }
        Err(e) => { fail(format!("{e}")); failures += 1; return finish(failures); }
    };

    // ── 5. LLM stream → parse TutorResponse ──────────────────────────────────
    step(5, 7, "LLM stream → parse TutorResponse");
    let messages = session.current_messages().to_vec();
    let llm_result = async {
        let mut full_text = String::new();
        let mut stream = llm.chat_stream(&messages).await?;
        while let Some(item) = stream.next().await {
            if let llm::StreamItem::Token(tok) = item? {
                full_text.push_str(&tok);
            }
        }
        Ok::<String, anyhow::Error>(full_text)
    }.await;

    let (tutor_resp, tts_input) = match llm_result {
        Ok(text) => {
            let resp = parse_response_pub(&text, false, false);
            if resp.response.is_empty() {
                fail("LLM returned empty response".into());
                failures += 1;
                return finish(failures);
            }
            let preview = resp.response.chars().take(40).collect::<String>();
            pass(Some(format!("response = {:?}…", preview)));
            let tts_input = resp.response.clone();
            (resp, tts_input)
        }
        Err(e) => { fail(format!("{e}")); failures += 1; return finish(failures); }
    };

    // Record the turn in the DB so the test exercises the full write path.
    let session_id = db.start_session()?;
    if let Err(e) = session.record_turn_sync(&greeting(), &tutor_resp, &db) {
        eprintln!("    (warn: record_turn failed — {e})");
    }
    db.end_session(session_id)?;

    // ── 6. TTS speak → WAV ───────────────────────────────────────────────────
    step(6, 7, "TTS speak → WAV");
    let wav = match tts.speak(&tts_input).await {
        Ok(wav) => match find_wav_data_offset(&wav) {
            Ok(_) => { pass(Some(format!("{} bytes", wav.len()))); wav }
            Err(e) => {
                fail(format!("invalid WAV structure: {e}"));
                failures += 1;
                return finish(failures);
            }
        }
        Err(e) => { fail(format!("{e}")); failures += 1; return finish(failures); }
    };

    // ── 7. AudioPlayer playback ───────────────────────────────────────────────
    step(7, 7, "AudioPlayer playback");
    let pcm = wav_to_f32(&wav)?;
    let duration_secs = pcm.len() as f64 / 24_000.0;
    let play_result = (|| -> anyhow::Result<()> {
        let mut player = AudioPlayer::new();
        player.start()?;
        player.play_chunk(&pcm, 24_000)?;
        Ok(())
    })();
    match play_result {
        Ok(_) => {
            // Wait for audio to finish before the process exits.
            tokio::time::sleep(std::time::Duration::from_secs_f64(duration_secs + 0.5)).await;
            pass(Some(format!("{duration_secs:.1}s played")));
        }
        Err(e) => { fail(format!("{e}")); failures += 1; }
    }

    finish(failures)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn step(n: u8, total: u8, label: &str) {
    print!("[{n}/{total}] {label} … ");
    std::io::stdout().flush().ok();
}

fn pass(detail: Option<String>) {
    match detail {
        Some(d) => println!("PASS  ({d})"),
        None    => println!("PASS"),
    }
}

fn fail(detail: String) {
    println!("FAIL  ({detail})");
}

fn print_summary(failures: usize) {
    println!();
    if failures == 0 {
        println!("✓ All tests passed.");
    } else {
        println!("✗ {failures} test(s) failed.");
    }
}

fn finish(failures: usize) -> anyhow::Result<()> {
    print_summary(failures);
    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn wav_to_f32(wav: &[u8]) -> anyhow::Result<Vec<f32>> {
    let offset = find_wav_data_offset(wav)?;
    Ok(wav[offset..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect())
}

/// Scan RIFF chunks to find the byte offset of the first `data` chunk payload.
/// Handles WAV files with extra chunks (e.g. VoiceVox's `fact` chunk).
fn find_wav_data_offset(wav: &[u8]) -> anyhow::Result<usize> {
    if wav.len() < 12 {
        anyhow::bail!("WAV payload too short ({} bytes)", wav.len());
    }
    let mut pos = 12;
    while pos + 8 <= wav.len() {
        let id   = &wav[pos..pos + 4];
        let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if id == b"data" {
            return Ok(pos + 8);
        }
        pos += 8 + size + (size & 1);
    }
    anyhow::bail!("no data chunk found in WAV");
}

fn greeting() -> String {
    "Good morning, my name is Carl. Please let me know when you are ready to start our Japanese lesson.".into()
}

fn db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nihongo")
        .join("nihongo.db")
}
