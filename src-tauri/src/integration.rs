//! Integration test binary — proves the full pipeline works:
//!   AudioManager → Vec<f32> chunks → Transcriber → printed transcript
//!
//! Run with:
//!  cargo run --bin integration -- ~/llm_models/whisper/ggml-large-v3-turbo.bin
//!
//! Speak into your mic. Each detected turn will be transcribed and printed.
//! Ctrl+C to stop.

use anyhow::{anyhow, Result};
use audio_engine::{AudioManager, EngineConfig};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use transcriber::{Transcriber, TranscriberConfig};
use std::{thread, time::{Duration, Instant}};

struct TranscribeRequest {
    audio: Vec<f32>,
    reply: oneshot::Sender<Result<String, String>>,
}

/// Spawn a dedicated std::thread owning one Transcriber instance.
/// Returns a sender to submit transcription requests.
fn spawn_transcriber_thread(
    label: &'static str,
    model_path: String,
) -> mpsc::Sender<TranscribeRequest> {
    let (tx, mut rx) = mpsc::channel::<TranscribeRequest>(8);

    thread::spawn(move || {
        let transcriber = match Transcriber::new(TranscriberConfig {
            model_path,
            language: "auto".into(),
            translate: false,
            initial_prompt: Some(
                "English and Japanese conversation. 日本語と英語の会話。"
                    .into(),
            ),
        }) {
            Ok(t) => {
                println!("[{}] model loaded ✓", label);
                t
            }
            Err(e) => {
                eprintln!("[{}] failed to load model: {}", label, e);
                return;
            }
        };

        while let Some(req) = rx.blocking_recv() {
            let result = transcriber
                .transcribe_to_text(&req.audio)
                .map_err(|e| e.to_string());
            let _ = req.reply.send(result);
        }

        println!("[{}] thread shutting down", label);
    });

    tx
}

#[tokio::main]
async fn main() -> Result<()> {
    let model_path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!(
            "Usage: cargo run --bin integration -- <path-to-model>"
        ))?;

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" Nihongo Tutor — Pipeline Integration Test (M4 Pro)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" Loading two Whisper instances in parallel...\n");

    // Load both transcribers simultaneously on separate threads.
    // On M4 Pro with 48GB this is trivial — ~1.1GB total for q5_0 turbo.
    let partial_tx = spawn_transcriber_thread("partial ", model_path.clone());
    let turnend_tx = spawn_transcriber_thread("turn_end", model_path.clone());

    // Give threads a moment to load — model loading is parallel here.
    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("\n Starting microphone...");

    let manager = AudioManager::new();
    let config = EngineConfig {
        silence_threshold: Duration::from_millis(600),
        vad_sensitivity: 0.5,
        partial_chunk_samples: 16_000, // emit partial every ~1s of speech
        ..Default::default()
    };

    let streams = manager.start(config)?;
    let mut partial_stream = streams.partial;
    let mut turn_stream = streams.turn_end;

    println!(" Microphone active ✓");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" Speak — partials appear while speaking, final on pause.\n");

    // ── Partial stream task ───────────────────────────────────────────────
    // Runs on its own tokio task. Drops stale partials if transcriber is busy.
    let partial_task = tokio::spawn(async move {
        while let Some(audio) = partial_stream.next().await {
            let duration_s = audio.len() as f32 / 16_000.0;

            let (reply_tx, reply_rx) = oneshot::channel();

            // Non-blocking — if the channel is full, skip this partial.
            // We'd rather drop a stale partial than queue up a backlog.
            if partial_tx.try_send(TranscribeRequest { audio, reply: reply_tx }).is_err() {
                eprintln!("  [partial] transcriber busy — dropping stale chunk");
                continue;
            }

            let t = Instant::now();
            match reply_rx.await {
                Ok(Ok(text)) if !text.trim().is_empty() => {
                    println!("  ◌ partial ({:.1}s) [{:.0?}]: {}", duration_s, t.elapsed(), text);
                }
                Ok(Ok(_)) => {} // silence chunk — ignore
                Ok(Err(e)) => eprintln!("  [partial] error: {}", e),
                Err(_)     => eprintln!("  [partial] reply channel dropped"),
            }
        }
    });

    // ── Turn end stream ───────────────────────────────────────────────────
    // Runs on the main task. This is the authoritative transcript → LLM feed.
    let mut turn_count = 0u32;

    while let Some(audio) = turn_stream.next().await {
        turn_count += 1;
        let duration_s = audio.len() as f32 / 16_000.0;

        println!("\n[Turn {:>3}] {:.2}s — final transcription...", turn_count, duration_s);

        let (reply_tx, reply_rx) = oneshot::channel();
        if turnend_tx.send(TranscribeRequest { audio, reply: reply_tx }).await.is_err() {
            eprintln!("  [turn_end] transcriber thread stopped");
            break;
        }

        let t = Instant::now();
        match reply_rx.await {
            Ok(Ok(text)) if text.trim().is_empty() => {
                println!("  ✓ (no speech detected)\n");
            }
            Ok(Ok(text)) => {
                println!("  ✓ [{:.0?}] \"{}\"\n", t.elapsed(), text);
                println!("  → ready for LLM\n");
            }
            Ok(Err(e)) => eprintln!("  ✗ error: {}\n", e),
            Err(_)     => eprintln!("  ✗ reply channel dropped\n"),
        }

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    partial_task.abort();
    println!("\n[Done] {} turns transcribed.", turn_count);
    Ok(())
}