use anyhow::{anyhow, Result};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use std::thread;

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
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            vad_window: 512,
            silence_threshold: Duration::from_millis(600),
            vad_sensitivity: 0.5,
            partial_chunk_samples: 16_000, // emit partial every ~1s of speech
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
}

pub struct AudioManager {
    is_running:           Arc<AtomicBool>,
    muted:                Arc<AtomicBool>,
    barge_in:             Arc<AtomicBool>,
    silence_threshold_ms: Arc<AtomicU64>,
    silence_default_ms:   Arc<AtomicU64>,
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
        }
    }

    pub fn audio_is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Discard all incoming audio — call before TTS playback to prevent echo.
    pub fn mute(&self) {
        self.muted.store(true, Ordering::SeqCst);
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

        self.is_running.store(true, Ordering::SeqCst);
        let running      = self.is_running.clone();
        let muted        = self.muted.clone();
        let barge_in     = self.barge_in.clone();
        let threshold_ms = self.silence_threshold_ms.clone();

        thread::spawn(move || {
            if let Err(e) = Self::run_loop(partial_tx, turn_tx, running, muted, barge_in, threshold_ms, config) {
                eprintln!("[manager] loop error: {}", e);
            }
        });

        Ok(AudioStreams {
            partial: ReceiverStream::new(partial_rx),
            turn_end: ReceiverStream::new(turn_rx),
        })
    }

    fn run_loop(
        partial_tx: mpsc::Sender<Vec<f32>>,
        turn_tx: mpsc::Sender<Vec<f32>>,
        running: Arc<AtomicBool>,
        muted: Arc<AtomicBool>,
        barge_in: Arc<AtomicBool>,
        threshold_ms: Arc<AtomicU64>,
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

        while running.load(Ordering::SeqCst) {
            let is_muted = muted.load(Ordering::SeqCst);

            // On unmute: flush any speech captured during TTS as the next queued turn.
            if prev_muted && !is_muted && !barge_buffer.is_empty() {
                let audio = std::mem::take(&mut barge_buffer);
                barge_in_turn = false;
                barge_remainder.clear();
                if turn_tx.blocking_send(audio).is_err() {
                    return Err(anyhow!("turn_end receiver dropped"));
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

                Self::align_windows(&mut mono_16k, &mut barge_remainder, config.vad_window);

                for chunk in mono_16k.chunks_exact(config.vad_window) {
                    if barge_vad.is_speech(chunk.to_vec(), config.vad_sensitivity) {
                        if !barge_in_turn {
                            barge_in_turn = true;
                            barge_buffer.clear();
                            // Signal the pipeline to stop TTS early.
                            barge_in.store(true, Ordering::SeqCst);
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

            Self::align_windows(&mut mono_16k, &mut remainder, config.vad_window);

            for chunk in mono_16k.chunks_exact(config.vad_window) {
                let is_speech = vad.is_speech(chunk.to_vec(), config.vad_sensitivity);

                if is_speech {
                    if !in_turn {
                        in_turn = true;
                        speech_buffer.clear();
                        samples_emitted_as_partial = 0;
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
                        let full_audio = std::mem::take(&mut speech_buffer);
                        samples_emitted_as_partial = 0;
                        in_turn = false;

                        if turn_tx.blocking_send(full_audio).is_err() {
                            return Err(anyhow!("turn_end receiver dropped"));
                        }
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