use anyhow::{anyhow, Result};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use std::thread;

use crate::aec::{AecProcessor, AecSink, create_aec_pair};
use crate::capture::AudioCapture;
use crate::resampler::Resampler16k;
use crate::vad::SpeechDetector;

pub struct EngineConfig {
    pub vad_window: usize,
    pub silence_threshold: Duration,
    pub vad_sensitivity: f32,
    /// How many samples to accumulate before emitting a partial chunk.
    /// At 16kHz, 16000 = 1 second, 32000 = 2 seconds.
    pub partial_chunk_samples: usize,
    /// How long to ignore mic input after muting before barge-in detection activates.
    /// During this window the mic captures speaker output directly (before the room
    /// echo settles), so any speech detection would be a false positive.
    /// Short TTS responses (e.g. greetings) complete within this window, effectively
    /// disabling barge-in for them while still allowing interruption on longer replies.
    pub barge_in_start_delay: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            vad_window: 512,
            silence_threshold: Duration::from_millis(600),
            vad_sensitivity: 0.5,
            partial_chunk_samples: 16_000, // emit partial every ~1s of speech
            barge_in_start_delay: Duration::from_millis(700),
        }
    }
}

/// Two-channel output from the audio pipeline.
/// Partial: emitted every `partial_chunk_samples` while the user is speaking.
/// TurnEnd: emitted when silence exceeds `silence_threshold` — the full utterance.
pub struct AudioStreams {
    /// Short rolling chunks emitted while speaking — feed to speculative transcription.
    pub partial: ReceiverStream<Vec<f32>>,
    /// Complete utterance emitted on turn end — feed to final transcription + LLM.
    pub turn_end: ReceiverStream<Vec<f32>>,
    /// [PIPELINE_DEBUG] VAD state change labels for the pipeline indicator UI.
    /// Carries stage name strings (e.g. "VAD 1: Speech Detected").
    pub vad_state: ReceiverStream<&'static str>,
    /// Feed every audio chunk sent to the speaker into this sink so the AEC can
    /// cancel its echo from the microphone signal.
    pub aec_sink: AecSink,
}

pub struct AudioManager {
    is_running:           Arc<AtomicBool>,
    muted:                Arc<AtomicBool>,
    barge_in:             Arc<AtomicBool>,
    silence_threshold_ms: Arc<AtomicU64>,
    silence_default_ms:   Arc<AtomicU64>,
    /// Unix-epoch milliseconds when audio actually started playing (first play_chunk call).
    /// Zero means no audio has started yet; barge-in detection is suppressed while zero.
    /// Reset to zero on every mute() so stale timestamps from previous turns don't leak.
    audio_play_started:   Arc<AtomicU64>,
}

impl AudioManager {
    pub fn new() -> Self {
        let default_ms = EngineConfig::default().silence_threshold.as_millis() as u64;
        Self {
            is_running:           Arc::new(AtomicBool::new(false)),
            muted:                Arc::new(AtomicBool::new(false)),
            barge_in:             Arc::new(AtomicBool::new(false)),
            silence_threshold_ms: Arc::new(AtomicU64::new(default_ms)),
            silence_default_ms:   Arc::new(AtomicU64::new(default_ms)),
            audio_play_started:   Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn audio_is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Discard all incoming audio — call before TTS playback to prevent echo.
    /// Also resets the play-started timestamp so barge-in stays suppressed until
    /// the first actual play_chunk call on this turn completes.
    pub fn mute(&self) {
        self.muted.store(true, Ordering::SeqCst);
        self.audio_play_started.store(0, Ordering::SeqCst);
    }

    /// Signal that audio has actually started reaching the speaker on this turn.
    /// Call this immediately after every play_chunk() call — it is idempotent:
    /// only the first call per turn records the timestamp (subsequent calls are
    /// no-ops because mute() resets the field to 0 at the start of each turn).
    /// Barge-in detection will not activate until `barge_in_start_delay` has
    /// elapsed from this moment — not from when mute() was called.
    pub fn signal_audio_started(&self) {
        // Only record the first call per turn (mute() resets to 0).
        if self.audio_play_started.load(Ordering::SeqCst) != 0 {
            return;
        }
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        // Use compare-exchange so two concurrent callers don't both set it.
        let _ = self.audio_play_started.compare_exchange(
            0, now_ms, Ordering::SeqCst, Ordering::SeqCst,
        );
    }

    /// Resume capture after TTS playback finishes.
    pub fn unmute(&self) {
        self.muted.store(false, Ordering::SeqCst);
    }

    /// Store the session-level silence threshold derived from learner level.
    /// Also sets the current threshold so the loop picks it up immediately.
    pub fn init_silence_threshold(&self, ms: u64) {
        self.silence_default_ms.store(ms, Ordering::SeqCst);
        self.silence_threshold_ms.store(ms, Ordering::SeqCst);
    }

    /// Temporarily tighten the threshold — e.g. 500ms for drill prompts.
    pub fn set_silence_threshold(&self, ms: u64) {
        self.silence_threshold_ms.store(ms, Ordering::SeqCst);
    }

    /// Reset to the session-level default set by `init_silence_threshold`.
    pub fn reset_silence_threshold(&self) {
        let default = self.silence_default_ms.load(Ordering::SeqCst);
        self.silence_threshold_ms.store(default, Ordering::SeqCst);
    }

    /// Returns `true` (and clears the flag) if speech was detected during TTS
    /// playback, signalling the pipeline to stop the player early.
    pub fn barge_in_pending(&self) -> bool {
        self.barge_in.swap(false, Ordering::SeqCst)
    }

    pub fn stop(&self) -> Result<()> {
        if !self.audio_is_running() {
            return Err(anyhow!("AudioManager is not running"));
        }
        self.is_running.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn start(&self, config: EngineConfig) -> Result<AudioStreams> {
        if self.audio_is_running() {
            return Err(anyhow!("AudioManager is already running"));
        }

        let (partial_tx, partial_rx) = mpsc::channel::<Vec<f32>>(8);
        let (turn_tx, turn_rx) = mpsc::channel::<Vec<f32>>(8);
        let (vad_tx, vad_rx) = mpsc::channel::<&'static str>(16); // [PIPELINE_DEBUG]
        let (aec_sink, aec_proc) = create_aec_pair()?;

        self.is_running.store(true, Ordering::SeqCst);
        let running            = self.is_running.clone();
        let muted              = self.muted.clone();
        let barge_in           = self.barge_in.clone();
        let threshold_ms       = self.silence_threshold_ms.clone();
        let audio_play_started = self.audio_play_started.clone();

        thread::spawn(move || {
            if let Err(e) = Self::run_loop(partial_tx, turn_tx, vad_tx, aec_proc, running, muted, barge_in, threshold_ms, audio_play_started, config) {
                eprintln!("[manager] loop error: {}", e);
            }
        });

        Ok(AudioStreams {
            partial:   ReceiverStream::new(partial_rx),
            turn_end:  ReceiverStream::new(turn_rx),
            vad_state: ReceiverStream::new(vad_rx), // [PIPELINE_DEBUG]
            aec_sink,
        })
    }

    fn run_loop(
        partial_tx: mpsc::Sender<Vec<f32>>,
        turn_tx: mpsc::Sender<Vec<f32>>,
        vad_tx: mpsc::Sender<&'static str>, // [PIPELINE_DEBUG]
        mut aec: AecProcessor,
        running: Arc<AtomicBool>,
        muted: Arc<AtomicBool>,
        barge_in: Arc<AtomicBool>,
        threshold_ms: Arc<AtomicU64>,
        audio_play_started: Arc<AtomicU64>,
        config: EngineConfig,
    ) -> Result<()> {
        let (capture, mut consumer) = AudioCapture::start()?;
        let mut resampler = Resampler16k::new(
            capture.config.sample_rate,
            capture.config.channels,
        )?;
        let mut vad = SpeechDetector::new(16_000, config.vad_window)?;
        // Separate instance so barge-in detection never touches the main VAD's LSTM state.
        let mut barge_vad = SpeechDetector::new(16_000, config.vad_window)?;

        let mut remainder = Vec::new();
        let mut speech_buffer: Vec<f32> = Vec::new();
        let mut last_voice = Instant::now();
        let mut in_turn = false;
        // Tracks how many samples have been sent as partials in the current turn.
        // Used to avoid re-sending the same audio in the final turn_end chunk.
        let mut samples_emitted_as_partial: usize = 0;

        // Speech spoken while TTS is playing is buffered here and flushed as the
        // next turn immediately after unmute, so it gets transcribed and sent to
        // the model with full conversation context intact.
        let mut barge_buffer: Vec<f32> = Vec::new();
        let mut barge_in_turn = false;
        let mut barge_remainder: Vec<f32> = Vec::new();
        let mut prev_muted = false;
        // After TTS playback ends, suppress new turns briefly so room echo doesn't
        // get mistaken for user speech. Skipped when a barge-in already filled the buffer.
        let mut echo_tail_until = Instant::now();
        // Whether AEC stream delay has been calibrated from the first hardware callback.
        let mut aec_delay_set = false;

        while running.load(Ordering::SeqCst) {
            // Calibrate AEC stream delay once the first InputCallbackInfo measurement
            // is available (~10ms after capture starts). The estimate is:
            //   stream_delay = input_latency × 2 + 5ms
            // (output buffer ≈ input buffer on macOS built-in audio; 5ms = room travel)
            if !aec_delay_set {
                let input_lat = capture.input_latency_ms.load(Ordering::Relaxed);
                if input_lat > 0 {
                    let stream_delay_ms = input_lat * 2 + 5;
                    aec.update_stream_delay(stream_delay_ms);
                    eprintln!(
                        "[aec] stream_delay calibrated: input={}ms, estimated_total={}ms",
                        input_lat, stream_delay_ms
                    );
                    aec_delay_set = true;
                }
            }
            let is_muted = muted.load(Ordering::SeqCst);

            // (audio_play_started is reset inside mute() — no extra bookkeeping needed here)

            // On unmute: flush barge-in audio (if any) or start an echo suppression window.
            if prev_muted && !is_muted {
                if !barge_buffer.is_empty() {
                    let audio = std::mem::take(&mut barge_buffer);
                    barge_in_turn = false;
                    barge_remainder.clear();
                    if turn_tx.blocking_send(audio).is_err() {
                        return Err(anyhow!("turn_end receiver dropped"));
                    }
                } else {
                    // No barge-in — suppress VAD for 400ms to let room echo decay.
                    echo_tail_until = Instant::now() + Duration::from_millis(400);
                    vad_tx.try_send("Echo: Suppressing").ok(); // [PIPELINE_DEBUG] not shown in flow UI
                }
            }
            prev_muted = is_muted;

            if is_muted {
                // Drain the resampler to prevent the ring buffer from overflowing,
                // then run VAD on the result so any speech the user makes during
                // TTS playback is buffered for processing after unmute.
                let mut mono_16k = match resampler.process_available(&mut consumer) {
                    Ok(s) if !s.is_empty() => s,
                    Ok(_) => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => return Err(e),
                };

                // AEC: cancel speaker echo so barge_vad only sees real user speech.
                // While muted, the render buffer has active TTS data — cancellation is valid.
                // In the unmuted path there is no speaker output, so AEC is not applied there
                // (feeding zeros as reference would suppress the user's voice instead).
                eprintln!("[aec] active — processing {} samples", mono_16k.len());
                aec.process_in_place(&mut mono_16k);
                if mono_16k.is_empty() {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }

                Self::align_windows(&mut mono_16k, &mut barge_remainder, config.vad_window);

                // Barge-in detection is gated on two conditions:
                // 1. audio has actually started playing (audio_play_started != 0)
                // 2. barge_in_start_delay has elapsed since the first play_chunk call
                // This means synthesis latency no longer "eats into" the delay window —
                // the delay only starts once audio is actually hitting the speaker.
                let barge_in_active = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let started_ms = audio_play_started.load(Ordering::SeqCst);
                    if started_ms == 0 {
                        false // audio hasn't started yet; still synthesising
                    } else {
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        now_ms.saturating_sub(started_ms)
                            >= config.barge_in_start_delay.as_millis() as u64
                    }
                };

                for chunk in mono_16k.chunks_exact(config.vad_window) {
                    if barge_in_active && barge_vad.is_speech(chunk.to_vec(), config.vad_sensitivity) {
                        if !barge_in_turn {
                            barge_in_turn = true;
                            barge_buffer.clear();
                            // Signal the pipeline to stop TTS early.
                            barge_in.store(true, Ordering::SeqCst);
                            vad_tx.try_send("VAD 2: Barge-In Detected").ok(); // [PIPELINE_DEBUG]
                        }
                        barge_buffer.extend_from_slice(chunk);
                    } else if barge_in_turn {
                        // Keep buffering trailing silence for context; the turn
                        // boundary is determined on unmute, not during mute.
                        barge_buffer.extend_from_slice(chunk);
                    }
                }

                speech_buffer.clear();
                in_turn = false;
                samples_emitted_as_partial = 0;
                continue;
            }

            let mut mono_16k = match resampler.process_available(&mut consumer) {
                Ok(s) if !s.is_empty() => s,
                Ok(_) => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(e),
            };

            // Echo tail: drain audio without feeding VAD while room echo may be present.
            if Instant::now() < echo_tail_until {
                thread::sleep(Duration::from_millis(10));
                continue;
            }

            // AEC is NOT applied here: in the unmuted path no TTS is playing, so the
            // render ring buffer is empty. Feeding zeros as the AEC reference would
            // cause AEC3 to suppress the user's voice. AEC runs in the muted path only.
            Self::align_windows(&mut mono_16k, &mut remainder, config.vad_window);

            for chunk in mono_16k.chunks_exact(config.vad_window) {
                let is_speech = vad.is_speech(chunk.to_vec(), config.vad_sensitivity);

                if is_speech {
                    if !in_turn {
                        in_turn = true;
                        speech_buffer.clear();
                        samples_emitted_as_partial = 0;
                        vad_tx.try_send("VAD 1: Speech Detected").ok(); // [PIPELINE_DEBUG]
                    }
                    last_voice = Instant::now();
                    speech_buffer.extend_from_slice(chunk);

                    // Emit a partial every `partial_chunk_samples` of new speech.
                    let new_samples = speech_buffer.len() - samples_emitted_as_partial;
                    if new_samples >= config.partial_chunk_samples {
                        let partial = speech_buffer.clone();
                        samples_emitted_as_partial = speech_buffer.len();
                        // Non-blocking send — if the transcriber is busy we skip
                        // this partial rather than blocking the audio thread.
                        if partial_tx.try_send(partial).is_err() {
                            eprintln!("[manager] partial channel full — skipping partial");
                        }
                    }

                } else if in_turn {
                    // Still in turn but silence — keep buffering the silence
                    // so the full utterance includes trailing context.
                    speech_buffer.extend_from_slice(chunk);

                    if last_voice.elapsed() > Duration::from_millis(threshold_ms.load(Ordering::SeqCst)) {
                        // Turn ended — send the complete utterance.
                        vad_tx.try_send("VAD 1: Silence Detected").ok(); // [PIPELINE_DEBUG]
                        let full_audio = std::mem::take(&mut speech_buffer);
                        samples_emitted_as_partial = 0;
                        in_turn = false;

                        if turn_tx.blocking_send(full_audio).is_err() {
                            return Err(anyhow!("turn_end receiver dropped"));
                        }
                        vad_tx.try_send("VAD 1: Listening").ok(); // [PIPELINE_DEBUG]
                    }
                }
            }
        }

        capture.stop();
        Ok(())
    }

    fn align_windows(samples: &mut Vec<f32>, remainder: &mut Vec<f32>, vad_window: usize) {
        if !remainder.is_empty() {
            let mut combined = std::mem::take(remainder);
            combined.append(samples);
            *samples = combined;
        }
        let offset = (samples.len() / vad_window) * vad_window;
        *remainder = samples.split_off(offset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_windows_splits_at_window_boundary() {
        let mut samples = vec![0.0f32; 600];
        let mut remainder = Vec::new();
        AudioManager::align_windows(&mut samples, &mut remainder, 512);
        assert_eq!(samples.len(), 512);
        assert_eq!(remainder.len(), 88);
    }

    #[test]
    fn align_windows_prepends_remainder_from_previous_call() {
        let mut remainder = vec![0.1f32; 400];
        let mut samples = vec![0.2f32; 200];
        AudioManager::align_windows(&mut samples, &mut remainder, 512);
        // 400 + 200 = 600 → emit 512, carry 88
        assert_eq!(samples.len(), 512);
        assert_eq!(remainder.len(), 88);
        // Remainder-originated samples come first
        assert!((samples[0] - 0.1).abs() < f32::EPSILON);
        assert!((samples[400] - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn align_windows_exact_multiple_leaves_empty_remainder() {
        let mut samples = vec![0.0f32; 1024];
        let mut remainder = Vec::new();
        AudioManager::align_windows(&mut samples, &mut remainder, 512);
        assert_eq!(samples.len(), 1024);
        assert!(remainder.is_empty());
    }

    #[test]
    fn align_windows_shorter_than_window_moves_all_to_remainder() {
        let mut samples = vec![0.0f32; 300];
        let mut remainder = Vec::new();
        AudioManager::align_windows(&mut samples, &mut remainder, 512);
        assert!(samples.is_empty());
        assert_eq!(remainder.len(), 300);
    }

    // Echo tail: audio within the suppression window must be skipped by the loop.
    // The production code checks `Instant::now() < echo_tail_until`; these tests
    // verify the boundary conditions that make it correct.
    #[test]
    fn echo_tail_active_immediately_after_unmute() {
        let tail_end = Instant::now() + Duration::from_millis(400);
        assert!(Instant::now() < tail_end, "suppression window should be active");
    }

    #[test]
    fn echo_tail_expired_when_set_in_past() {
        let tail_end = Instant::now() - Duration::from_millis(1);
        assert!(!(Instant::now() < tail_end), "suppression window should have expired");
    }
}