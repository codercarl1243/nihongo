/// TTS fixture generator — pre-synthesise multi-sentence audio for manual pipeline testing.
///
/// Requires the Python sidecar running on localhost:8091.
/// Start it with:  cd sidecar && bash start.sh
///
/// What it does:
///   1. Synthesises SENTENCES using BOTH the old sequential approach and the new
///      parallel dispatcher (Phase 5) and compares wall-clock synthesis time.
///   2. Saves each sentence as an individual WAV in test_fixtures/tts/.
///   3. Saves a full-session concatenated WAV (test_fixtures/tts/full_session.wav).
///   4. Plays the full session through AudioPlayer so you can hear the result.
///
/// Once the files are written you can re-run the playback step without the
/// sidecar by loading the saved WAVs directly:
///   cargo run --bin gen_tts_fixtures -- --play-saved

use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use audio_engine::AudioPlayer;
use llm::SidecarClient;

/// The sentences that make up a realistic N3-level tutor response.
/// Four sentences so the sequential-vs-parallel comparison is meaningful.
const SENTENCES: &[&str] = &[
    "いいですね！では、今日も日本語の練習を始めましょう。",
    "まず、自己紹介をしてみてください。",
    "たとえば、「わたしのなまえは＿＿です。どうぞよろしく。」と言ってみて。",
    "ゆっくりでいいですよ。準備ができたら話してください。",
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let play_saved = std::env::args().any(|a| a == "--play-saved");

    let fixtures_dir = fixtures_dir();
    std::fs::create_dir_all(&fixtures_dir)?;

    if play_saved {
        return play_from_disk(&fixtures_dir).await;
    }

    let llm = SidecarClient::new();

    // ── 1. Sidecar health ────────────────────────────────────────────────────
    step("Waiting for sidecar");
    llm.wait_until_ready(15).await?;
    done("ready");

    // ── 2. Sequential synthesis (baseline) ──────────────────────────────────
    println!("\n── Sequential synthesis (baseline) ──────────────────────────");
    let seq_start = Instant::now();
    let mut seq_wavs: Vec<Vec<u8>> = Vec::new();
    for (i, sentence) in SENTENCES.iter().enumerate() {
        let t = Instant::now();
        let wav = llm.speak(sentence).await?;
        println!("  sentence {}: {:>5} bytes  ({:.0}ms)", i + 1, wav.len(), t.elapsed().as_millis());
        seq_wavs.push(wav);
    }
    let seq_total = seq_start.elapsed();
    println!("  total synthesis time: {:.0}ms", seq_total.as_millis());

    // ── 3. Parallel synthesis (Phase 5 dispatcher) ───────────────────────────
    println!("\n── Parallel dispatch (Phase 5) ──────────────────────────────");
    let par_start = Instant::now();

    // Replicate the dispatcher + playback-loop pattern from handle_turn().
    let (wav_handle_tx, mut wav_handle_rx) =
        tokio::sync::mpsc::channel::<tokio::task::JoinHandle<anyhow::Result<Vec<u8>>>>(8);

    // Dispatcher: fires all TTS tasks immediately.
    let llm_d = llm.clone();
    let sentences: Vec<String> = SENTENCES.iter().map(|s| s.to_string()).collect();
    let dispatcher = tokio::spawn(async move {
        for sentence in sentences {
            let llm = llm_d.clone();
            let handle = tokio::spawn(async move { llm.speak(&sentence).await });
            if wav_handle_tx.send(handle).await.is_err() { break; }
        }
    });

    // Collect results in order, measuring time from dispatcher start.
    let mut par_wavs: Vec<Vec<u8>> = Vec::new();
    let mut prev_done = par_start;
    while let Some(handle) = wav_handle_rx.recv().await {
        let wav = handle.await.map_err(|e| anyhow::anyhow!("TTS task panicked: {e}"))??;
        let gap = prev_done.elapsed();
        println!(
            "  sentence {}: {:>5} bytes  (gap since prev: {:.0}ms, wall: {:.0}ms)",
            par_wavs.len() + 1,
            wav.len(),
            gap.as_millis(),
            par_start.elapsed().as_millis(),
        );
        par_wavs.push(wav);
        prev_done = Instant::now();
    }
    dispatcher.abort();
    let par_total = par_start.elapsed();
    println!("  total synthesis time: {:.0}ms", par_total.as_millis());

    // ── 4. Timing summary ────────────────────────────────────────────────────
    println!("\n── Timing summary ───────────────────────────────────────────");
    println!("  sequential : {:.0}ms", seq_total.as_millis());
    println!("  parallel   : {:.0}ms", par_total.as_millis());
    let saved_ms = seq_total.as_millis().saturating_sub(par_total.as_millis());
    println!("  saved      : {:.0}ms  ({:.1}%)",
        saved_ms,
        saved_ms as f64 / seq_total.as_millis() as f64 * 100.0,
    );

    // ── 5. Write WAV fixtures ────────────────────────────────────────────────
    // Use the parallel-synthesised wavs (same quality, just faster).
    println!("\n── Writing fixtures to {} ──", fixtures_dir.display());
    let mut combined_pcm: Vec<f32> = Vec::new();
    for (i, wav) in par_wavs.iter().enumerate() {
        let path = fixtures_dir.join(format!("sentence_{:02}.wav", i + 1));
        std::fs::write(&path, wav)?;
        println!("  wrote {}", path.display());
        combined_pcm.extend(wav_to_f32(wav)?);
    }
    let combined_path = fixtures_dir.join("full_session.wav");
    std::fs::write(&combined_path, pcm_to_wav(&combined_pcm, 24_000))?;
    println!("  wrote {}", combined_path.display());

    // ── 6. Playback ──────────────────────────────────────────────────────────
    println!("\n── Playing full session through AudioPlayer ─────────────────");
    play_pcm(&combined_pcm).await?;

    println!("\n✓ Done.  Re-run with --play-saved to play without the sidecar.\n");
    Ok(())
}

// ---------------------------------------------------------------------------
// --play-saved: load and play fixtures without the sidecar
// ---------------------------------------------------------------------------

async fn play_from_disk(fixtures_dir: &PathBuf) -> anyhow::Result<()> {
    let path = fixtures_dir.join("full_session.wav");
    anyhow::ensure!(path.exists(), "No saved fixture at {}. Run without --play-saved first.", path.display());
    let wav = std::fs::read(&path)?;
    println!("Playing {} ({} bytes)…", path.display(), wav.len());
    let pcm = wav_to_f32(&wav)?;
    play_pcm(&pcm).await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn play_pcm(pcm: &[f32]) -> anyhow::Result<()> {
    let duration_secs = pcm.len() as f64 / 24_000.0;
    let mut player = AudioPlayer::new();
    player.start()?;
    player.play_chunk(pcm, 24_000)?;
    println!("  playing {duration_secs:.1}s of audio…");
    tokio::time::sleep(Duration::from_secs_f64(duration_secs + 0.5)).await;
    println!("  done.");
    Ok(())
}

/// Decode a WAV file (PCM 16-bit LE, 44-byte header) to mono f32 samples.
fn wav_to_f32(wav: &[u8]) -> anyhow::Result<Vec<f32>> {
    anyhow::ensure!(wav.len() >= 44, "WAV too short ({} bytes)", wav.len());
    Ok(wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect())
}

/// Encode mono f32 samples as a minimal WAV (PCM 16-bit LE).
fn pcm_to_wav(pcm: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_samples = pcm.len() as u32;
    let data_len = num_samples * 2;
    let mut buf = Vec::with_capacity(44 + data_len as usize);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());       // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes());        // PCM
    buf.extend_from_slice(&1u16.to_le_bytes());        // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes()); // sample rate
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes());        // block align
    buf.extend_from_slice(&16u16.to_le_bytes());       // bits per sample
    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for &s in pcm {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_fixtures/tts")
}

fn step(label: &str) {
    print!("{label} … ");
    std::io::stdout().flush().ok();
}

fn done(detail: &str) {
    println!("{detail}");
}
