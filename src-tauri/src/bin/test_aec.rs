/// AEC + BargeInDetector integration tests — pure in-memory, no hardware required.
///
/// Expected output:
///
///   [1/6] AEC pair creation … PASS
///   [2/6] AEC cancels TTS echo below barge-in gate (rms < 0.025) … PASS
///   [3/6] Real speech passes barge-in gate (rms > 0.025) … PASS
///   [4/6] BargeInDetector suppressed during start delay … PASS
///   [5/6] BargeInDetector confirms after streak … PASS
///   [6/6] TTS echo → AEC → gate → no barge-in … PASS
///
///   ✓ All 6 tests passed.
///
/// Run with:
///   cargo run --bin test_aec --manifest-path src-tauri/Cargo.toml

use std::f32::consts::PI;
use audio_engine::{
    BargeInDetector, BargeInState,
    aec::create_aec_pair,
};

const SAMPLE_RATE: usize = 16_000;
const BARGE_THRESHOLD: f32 = 0.025;
const CHUNK_SIZE: usize = 512;

fn sine_wave(seconds: f32, amplitude: f32) -> Vec<f32> {
    let n = (SAMPLE_RATE as f32 * seconds) as usize;
    (0..n)
        .map(|i| amplitude * (2.0 * PI * 440.0 * i as f32 / SAMPLE_RATE as f32).sin())
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 0.0; }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

fn pass(n: usize, total: usize, desc: &str) {
    println!("[{n}/{total}] {desc} … PASS");
}

fn fail(n: usize, total: usize, desc: &str, detail: &str) {
    eprintln!("[{n}/{total}] {desc} … FAIL: {detail}");
}

fn main() {
    let total = 6;
    let mut failures = 0;

    // ── Test 1: AEC pair creation ────────────────────────────────────────────
    let n = 1;
    match create_aec_pair() {
        Ok(_) => pass(n, total, "AEC pair creation"),
        Err(e) => { fail(n, total, "AEC pair creation", &e.to_string()); failures += 1; }
    }

    // ── Test 2: AEC cancels TTS echo below barge-in gate ────────────────────
    let n = 2;
    {
        let desc = "AEC cancels TTS echo below barge-in gate (rms < 0.025)";
        match create_aec_pair() {
            Err(e) => { fail(n, total, desc, &e.to_string()); failures += 1; }
            Ok((sink, mut proc)) => {
                let reference = sine_wave(1.0, 0.3);
                sink.push(&reference, SAMPLE_RATE as u32);
                let mut mic = reference.clone();
                proc.process_in_place(&mut mic);
                let r = rms(&mic);
                if r < BARGE_THRESHOLD {
                    pass(n, total, &format!("{desc} (residual rms={r:.4})"));
                } else {
                    fail(n, total, desc, &format!("rms={r:.4} >= threshold={BARGE_THRESHOLD}"));
                    failures += 1;
                }
            }
        }
    }

    // ── Test 3: Real speech passes barge-in gate ─────────────────────────────
    let n = 3;
    {
        let desc = "Real speech passes barge-in gate (rms > 0.025)";
        match create_aec_pair() {
            Err(e) => { fail(n, total, desc, &e.to_string()); failures += 1; }
            Ok((sink, mut proc)) => {
                let silence = vec![0.0f32; SAMPLE_RATE / 2];
                sink.push(&silence, SAMPLE_RATE as u32);
                let mut user_speech = sine_wave(0.5, 0.3);
                proc.process_in_place(&mut user_speech);
                let r = rms(&user_speech);
                if r > BARGE_THRESHOLD {
                    pass(n, total, &format!("{desc} (rms={r:.4})"));
                } else {
                    fail(n, total, desc, &format!("rms={r:.4} <= threshold={BARGE_THRESHOLD}"));
                    failures += 1;
                }
            }
        }
    }

    // ── Test 4: BargeInDetector suppressed during start delay ────────────────
    let n = 4;
    {
        let desc = "BargeInDetector suppressed during start delay";
        let mut d = BargeInDetector::new(6, BARGE_THRESHOLD, 700);
        d.signal_audio_started(1000);
        let chunk = vec![0.3f32; CHUNK_SIZE]; // loud chunk
        // now_ms = 1001 — only 1ms elapsed, delay is 700ms
        let state = d.process_chunk(&chunk, true, 1001);
        if matches!(state, BargeInState::Suppressed) {
            pass(n, total, desc);
        } else {
            fail(n, total, desc, "expected Suppressed but got a different state");
            failures += 1;
        }
    }

    // ── Test 5: BargeInDetector confirms after streak ────────────────────────
    let n = 5;
    {
        let desc = "BargeInDetector confirms after streak";
        let streak_required = 6u32;
        let mut d = BargeInDetector::new(streak_required, BARGE_THRESHOLD, 0);
        d.signal_audio_started(1);
        let chunk = vec![0.3f32; CHUNK_SIZE];
        let now_ms = 1u64;
        let mut confirmed = false;
        for _ in 0..streak_required {
            match d.process_chunk(&chunk, true, now_ms) {
                BargeInState::Confirmed => { confirmed = true; break; }
                _ => {}
            }
        }
        if confirmed {
            pass(n, total, desc);
        } else {
            fail(n, total, desc, &format!("did not confirm after {streak_required} loud speech chunks"));
            failures += 1;
        }
    }

    // ── Test 6: TTS echo → AEC → gate → no barge-in ─────────────────────────
    let n = 6;
    {
        let desc = "TTS echo → AEC → gate → no barge-in";
        match create_aec_pair() {
            Err(e) => { fail(n, total, desc, &e.to_string()); failures += 1; }
            Ok((sink, mut proc)) => {
                let mut d = BargeInDetector::new(6, BARGE_THRESHOLD, 0);
                d.signal_audio_started(1);

                let tts_audio = sine_wave(2.0, 0.3);
                sink.push(&tts_audio, SAMPLE_RATE as u32);

                let mut triggered = false;
                for chunk_start in (0..tts_audio.len()).step_by(CHUNK_SIZE) {
                    let end = chunk_start + CHUNK_SIZE;
                    if end > tts_audio.len() { break; }
                    let mut chunk = tts_audio[chunk_start..end].to_vec();
                    proc.process_in_place(&mut chunk);
                    if chunk.len() < CHUNK_SIZE { continue; }
                    if let BargeInState::Confirmed = d.process_chunk(&chunk[..CHUNK_SIZE], true, 1) {
                        triggered = true;
                        break;
                    }
                }
                if !triggered {
                    pass(n, total, desc);
                } else {
                    fail(n, total, desc, "barge-in was confirmed by TTS echo (should have been cancelled by AEC)");
                    failures += 1;
                }
            }
        }
    }

    // ── Summary ──────────────────────────────────────────────────────────────
    println!();
    if failures == 0 {
        println!("✓ All {total} tests passed.");
        std::process::exit(0);
    } else {
        eprintln!("✗ {failures}/{total} test(s) FAILED.");
        std::process::exit(1);
    }
}
