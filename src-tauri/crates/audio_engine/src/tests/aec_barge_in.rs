use std::f32::consts::PI;
use crate::aec::create_aec_pair;
use crate::barge_in::{BargeInDetector, BargeInState};

const SAMPLE_RATE: usize = 16_000;
const BARGE_THRESHOLD: f32 = 0.025;

fn sine_wave(seconds: f32, amplitude: f32) -> Vec<f32> {
    let n = (SAMPLE_RATE as f32 * seconds) as usize;
    (0..n)
        .map(|i| amplitude * (2.0 * PI * 440.0 * i as f32 / SAMPLE_RATE as f32).sin())
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Test 1 — AEC cancels its own echo below the barge-in gate.
///
/// Push TTS reference audio through AecSink, then feed the *same* signal as
/// mic input through AecProcessor. The cancellation residual must fall below
/// the barge-in RMS threshold, proving echo alone cannot trigger barge-in.
#[test]
fn aec_residual_is_below_barge_in_gate() {
    let (sink, mut proc) = create_aec_pair().expect("create_aec_pair failed");

    // Loud sine — simulates TTS output (rms ≈ 0.15 for amplitude 0.3 / √2 ≈ 0.21)
    let reference = sine_wave(1.0, 0.3);

    // Push all frames to the reference (speaker) path first.
    sink.push(&reference, SAMPLE_RATE as u32);

    // Now process the same signal as mic input — AEC should cancel it.
    let mut mic = reference.clone();
    proc.process_in_place(&mut mic);

    let output_rms = rms(&mic);
    assert!(
        output_rms < BARGE_THRESHOLD,
        "AEC residual rms={output_rms:.4} should be < {BARGE_THRESHOLD} (barge-in gate)"
    );
}

/// Test 2 — Real user speech is NOT suppressed below the gate.
///
/// Push silence as the TTS reference (nothing playing), then feed a loud sine
/// as mic input. AEC has no echo to cancel, so user speech must pass the gate.
#[test]
fn real_speech_is_not_suppressed_below_gate() {
    let (sink, mut proc) = create_aec_pair().expect("create_aec_pair failed");

    // Silence reference — no TTS audio playing
    let silence = vec![0.0f32; SAMPLE_RATE / 2];
    sink.push(&silence, SAMPLE_RATE as u32);

    // Loud mic signal — simulates genuine user speech
    let mut user_speech = sine_wave(0.5, 0.3);
    proc.process_in_place(&mut user_speech);

    let output_rms = rms(&user_speech);
    assert!(
        output_rms > BARGE_THRESHOLD,
        "User speech rms={output_rms:.4} should be > {BARGE_THRESHOLD} (should not be suppressed)"
    );
}

/// Test 3 — BargeInDetector + AEC pipeline (end-to-end, no hardware).
///
/// Cancelled echo chunks (rms < threshold) must never trigger Confirmed.
/// Real speech chunks (rms > threshold, is_speech=true) must trigger Confirmed.
#[test]
fn bargein_detector_aec_pipeline() {
    let (sink, mut proc) = create_aec_pair().expect("create_aec_pair failed");

    const STREAK: u32 = 6;
    const DELAY_MS: u64 = 0; // no delay so test runs immediately
    const CHUNK_SIZE: usize = 512;

    // Use timestamp 1 (non-zero) so is_active() sees a valid start time.
    let mut detector = BargeInDetector::new(STREAK, BARGE_THRESHOLD, DELAY_MS);
    detector.signal_audio_started(1);

    // ── Phase A: cancelled echo must not confirm ──────────────────────────
    let reference = sine_wave(0.5, 0.3);
    sink.push(&reference, SAMPLE_RATE as u32);

    let mut echo_chunks_confirmed = false;
    for chunk_start in (0..reference.len()).step_by(CHUNK_SIZE) {
        let end = (chunk_start + CHUNK_SIZE).min(reference.len());
        if end - chunk_start < CHUNK_SIZE {
            break;
        }
        let mut chunk = reference[chunk_start..end].to_vec();
        proc.process_in_place(&mut chunk);
        if chunk.len() < CHUNK_SIZE {
            continue;
        }
        match detector.process_chunk(&chunk[..CHUNK_SIZE], true, 1) {
            BargeInState::Confirmed => { echo_chunks_confirmed = true; break; }
            _ => {}
        }
    }
    assert!(!echo_chunks_confirmed, "cancelled echo should never confirm barge-in");

    // ── Phase B: real speech (no TTS reference) must confirm ─────────────
    let mut detector2 = BargeInDetector::new(STREAK, BARGE_THRESHOLD, DELAY_MS);
    detector2.signal_audio_started(1);

    let speech = sine_wave(1.0, 0.3);
    let mut confirmed = false;
    for chunk_start in (0..speech.len()).step_by(CHUNK_SIZE) {
        let end = chunk_start + CHUNK_SIZE;
        if end > speech.len() { break; }
        match detector2.process_chunk(&speech[chunk_start..end], true, 1) {
            BargeInState::Confirmed => { confirmed = true; break; }
            _ => {}
        }
    }
    assert!(confirmed, "real speech should trigger barge-in Confirmed after {STREAK} chunks");
}

/// Test 4 — TTS playback echo does not trigger barge-in (combined).
///
/// Simulates the production path: TTS audio pushed to AecSink, the same signal
/// treated as mic input through AecProcessor, result evaluated by BargeInDetector.
/// AudioPlayer is NOT used — it requires real CPAL hardware. Instead we push
/// the reference directly to AecSink, which is exactly what AudioPlayer does.
#[test]
fn tts_echo_does_not_trigger_bargein() {
    let (sink, mut proc) = create_aec_pair().expect("create_aec_pair failed");

    const STREAK: u32 = 6;
    const DELAY_MS: u64 = 0;
    const CHUNK_SIZE: usize = 512;

    let mut detector = BargeInDetector::new(STREAK, BARGE_THRESHOLD, DELAY_MS);
    detector.signal_audio_started(1);

    // 2-second TTS tone — simulates a complete TTS response
    let tts_audio = sine_wave(2.0, 0.3);
    sink.push(&tts_audio, SAMPLE_RATE as u32);

    // Feed the same audio as mic (room echo path) through AEC
    let mut confirmed = false;
    for chunk_start in (0..tts_audio.len()).step_by(CHUNK_SIZE) {
        let end = chunk_start + CHUNK_SIZE;
        if end > tts_audio.len() { break; }
        let mut chunk = tts_audio[chunk_start..end].to_vec();
        proc.process_in_place(&mut chunk);
        if chunk.len() < CHUNK_SIZE { continue; }
        match detector.process_chunk(&chunk[..CHUNK_SIZE], true, 1) {
            BargeInState::Confirmed => { confirmed = true; break; }
            _ => {}
        }
    }
    assert!(!confirmed, "TTS echo should not trigger barge-in after AEC cancellation");
}
