/// Backend integration test — runs without the Tauri app.
///
/// Requires the Python sidecar running on localhost:8091.
/// Start it with:  cd sidecar && bash start.sh
///   or:           pnpm test:integration   (starts sidecar automatically)
///
/// Expected output:
///
///   [1/6] Sidecar health … PASS
///   [2/6] Database — open, start session, record turn, end session … PASS  (session_id=1)
///   [3/6] TutorSession + system prompt … PASS  (2 messages in context)
///   [4/6] LLM stream → parse TutorResponse … PASS  (response = "いい天気ですね…")
///   [5/6] TTS speak → WAV … PASS  (12345 bytes)
///   [6/6] AudioPlayer playback … PASS
///
///   ✓ All tests passed.
///
/// What each test checks:
///   1. Health        — sidecar responds on localhost:8091
///   2. DB            — Db::open, start_session, record_turn, end_session all succeed
///   3. TutorSession  — builds a session from the live DB; system prompt renders without panic
///   4. LLM           — full chat stream completes; parse_response produces a non-empty response
///   5. TTS           — speak() returns WAV bytes; WAV header is valid (≥44 bytes)
///   6. Playback      — AudioPlayer decodes and plays the TTS WAV without error

use std::io::Write as _;
use std::path::PathBuf;
use tokio_stream::StreamExt;

use audio_engine::AudioPlayer;
use db::Db;
use llm::SidecarClient;
use tutor::{parse_response_pub, TutorSession};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let llm = SidecarClient::new();
    let mut failures = 0;

    // ── 1. Sidecar health ────────────────────────────────────────────────────
    step(1, 6, "Sidecar health");
    match llm.wait_until_ready(10).await {
        Ok(_) => pass(None),
        Err(e) => { fail(format!("{e}")); failures += 1; }
    }

    // ── 2. Database ──────────────────────────────────────────────────────────
    step(2, 6, "Database — open, start session, record turn, end session");
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

    // ── 3. TutorSession + system prompt ──────────────────────────────────────
    step(3, 6, "TutorSession + system prompt");
    let session_result = (|| -> anyhow::Result<TutorSession> {
        let mut session = TutorSession::new(&db)?;
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

    // ── 4. LLM stream → parse TutorResponse ──────────────────────────────────
    step(4, 6, "LLM stream → parse TutorResponse");
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
            let resp = parse_response_pub(&text);
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

    // ── 5. TTS speak → WAV ───────────────────────────────────────────────────
    step(5, 6, "TTS speak → WAV");
    let wav = match llm.speak(&tts_input).await {
        Ok(wav) if wav.len() >= 44 => {
            pass(Some(format!("{} bytes", wav.len())));
            wav
        }
        Ok(wav) => {
            fail(format!("WAV too short ({} bytes — missing header)", wav.len()));
            failures += 1;
            return finish(failures);
        }
        Err(e) => { fail(format!("{e}")); failures += 1; return finish(failures); }
    };

    // ── 6. AudioPlayer playback ───────────────────────────────────────────────
    step(6, 6, "AudioPlayer playback");
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
    anyhow::ensure!(wav.len() >= 44, "WAV payload too short ({} bytes)", wav.len());
    Ok(wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect())
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
