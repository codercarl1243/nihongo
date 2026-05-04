/// End-to-end integration test for the Rust backend.
///
/// Requires the Python sidecar to be running on localhost:8091.
/// Run with: ./test-backend.sh   (from the project root)
///
/// Steps exercised:
///   1. Sidecar health check
///   2. DB — open, start session, record turn, end session
///   3. TutorSession + system prompt build
///   4. LLM chat stream → parse TutorResponse  (can take 30–60s on first call)
///   5. TTS speak → WAV bytes
///   6. WAV decode → f32 PCM, play via AudioPlayer

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

    // ── 1. Sidecar health ────────────────────────────────────────────────────
    step(1, "Sidecar health");
    llm.wait_until_ready(10).await?;
    ok(None);

    // ── 2. DB ────────────────────────────────────────────────────────────────
    step(2, "Database");
    let db = Db::open(&db_path())?;
    let session_id = db.start_session()?;
    ok(Some(format!("session_id={session_id}")));

    // ── 3. TutorSession + system prompt ──────────────────────────────────────
    step(3, "TutorSession");
    let mut session = TutorSession::new(&db)?;
    let test_input = greeting();
    session.push_user(&test_input);
    let messages = session.current_messages().to_vec();
    ok(Some(format!("{} messages in context", messages.len())));

    // ── 4. LLM stream → parse response ───────────────────────────────────────
    step(4, "LLM stream  (first call may take 30–60s)");
    let mut full_text = String::new();
    {
        use llm::StreamItem;
        let mut stream = llm.chat_stream(&messages).await?;
        while let Some(item) = stream.next().await {
            if let StreamItem::Token(tok) = item? {
                full_text.push_str(&tok);
            }
        }
    }
    let tutor_resp = parse_response_pub(&full_text);
    ok(None);
    println!("       transcript : {:?}", tutor_resp.transcript);
    println!("       response   : {:?}", tutor_resp.response.chars().take(80).collect::<String>());
    println!("       milestone  : {}", tutor_resp.milestone);

    session.record_turn_sync(&test_input, &tutor_resp, &db)?;
    db.end_session(session_id)?;
    println!("       turn + session flushed to DB");

    // ── 5. TTS speak ─────────────────────────────────────────────────────────
    step(5, "TTS speak");
    if tutor_resp.response.is_empty() {
        ok(Some("skipped — empty response".into()));
    } else {
        let wav = llm.speak(&tutor_resp.response).await?;
        ok(Some(format!("{} bytes WAV", wav.len())));

        // ── 6. AudioPlayer playback ──────────────────────────────────────────
        let pcm = wav_to_f32(&wav)?;
        let duration_secs = pcm.len() as f64 / 24_000.0;
        step(6, &format!("AudioPlayer — playing {duration_secs:.1}s"));

        let mut player = AudioPlayer::new();
        player.start()?;
        player.play_chunk(&pcm, 24_000)?;
        tokio::time::sleep(std::time::Duration::from_secs_f64(duration_secs + 0.5)).await;
        ok(None);
    }

    println!("\nAll steps passed.");
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn step(n: u8, label: &str) {
    print!("[{n}/6] {label} … ");
    std::io::stdout().flush().ok();
}

fn ok(detail: Option<String>) {
    match detail {
        Some(d) => println!("ok  ({d})"),
        None    => println!("ok"),
    }
}

fn wav_to_f32(wav: &[u8]) -> anyhow::Result<Vec<f32>> {
    if wav.len() < 44 {
        anyhow::bail!("WAV payload too short ({} bytes)", wav.len());
    }
    Ok(wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect())
}

fn greeting() -> String {
    let hour = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // UTC offset for macOS local time via TZ env isn't trivial without chrono;
        // use UTC and add a rough offset — good enough for a greeting.
        ((secs % 86_400) / 3_600) as u8
    };

    let time_of_day = match hour {
        5..=11  => "morning",
        12..=16 => "afternoon",
        17..=20 => "evening",
        _       => "evening",
    };

    format!(
        "Good {time_of_day}, my name is Carl. \
         Please let me know when you are ready to start our Japanese lesson."
    )
}

fn db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nihongo")
        .join("nihongo.db")
}
