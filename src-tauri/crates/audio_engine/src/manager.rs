// src/manager.rs
use anyhow::{anyhow, Result};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use std::thread;

use crate::capture::AudioCapture;
use crate::resampler::Resampler16k;
use crate::vad::SpeechDetector;

pub struct EngineConfig {
    pub vad_window: usize,
    pub silence_threshold: Duration,
    pub vad_sensitivity: f32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            vad_window: 512,
            silence_threshold: Duration::from_millis(600),
            vad_sensitivity: 0.5,
        }
    }
}

/// Orchestrates the audio pipeline. 
/// 
/// The `AudioManager` manages the lifecycle of the microphone, 
/// resampling logic, and Voice Activity Detection (VAD).
pub struct AudioManager {
    is_running: Arc<AtomicBool>,
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn audio_is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
    
    /// Sends a signal to stop the background audio thread.
    /// This will gracefully close the microphone and the output stream.
    /// # Errors
    /// Returns an error if the engine is not currently running.
    pub fn stop(&self) -> Result<()> {
        if !self.audio_is_running() {
            return Err(anyhow!("AudioManager is not running"));
        }
        self.is_running.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Starts the background audio thread and returns an asynchronous 
    /// Stream of audio turns (Vec<f32>).
    /// 
    /// # Errors
    /// Returns an error if the microphone cannot be initialized or if the 
    /// engine is already running.
    pub fn start(&self, config: EngineConfig) -> Result<ReceiverStream<Vec<f32>>> {
        if self.audio_is_running() {
            return Err(anyhow!("AudioManager is already running"));
        }

        // 1. Generate the channel internally
        let (tx, rx) = mpsc::channel::<Vec<f32>>(32);
        
        self.is_running.store(true, Ordering::SeqCst);
        let running = self.is_running.clone();

        // 2. Pass the generated tx to the capture logic
        thread::spawn(move || {
            if let Err(e) = Self::run_loop(tx, running, config) {
                eprintln!("[manager] Loop error: {}", e);
            }
        });

        // 3. Return the receiver wrapped as a Stream
        Ok(ReceiverStream::new(rx))
    }

    fn run_loop(tx: mpsc::Sender<Vec<f32>>, running: Arc<AtomicBool>, config: EngineConfig) -> Result<()> {
        let (capture, mut consumer) = AudioCapture::start()?;
        let mut resampler = Resampler16k::new(capture.config.sample_rate, capture.config.channels)?;
        let mut vad = SpeechDetector::new(16_000, config.vad_window)?;

        let mut remainder = Vec::new();
        let mut speech_buffer = Vec::new();
        let mut last_voice = Instant::now();
        let mut in_turn = false;

        while running.load(Ordering::SeqCst) {
            let mut mono_16k: Vec<f32> = match resampler.process_available(&mut consumer) {
                Ok(s) if !s.is_empty() => s,
                Ok(_) => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(e),
            };

            Self::align_windows(&mut mono_16k, &mut remainder, config.vad_window);

            Self::process_vad_windows(
                &mono_16k,
                &mut vad,
                &mut speech_buffer,
                &mut last_voice,
                &mut in_turn,
                &tx,
                &config,
            )?;
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

    fn process_vad_windows(
        windows: &[f32],
        vad: &mut SpeechDetector,
        buffer: &mut Vec<f32>,
        last_voice: &mut Instant,
        in_turn: &mut bool,
        tx: &mpsc::Sender<Vec<f32>>,
        config: &EngineConfig
    ) -> Result<()> {
        for chunk in windows.chunks_exact(config.vad_window) {
            let is_speech = vad.is_speech(chunk.to_vec(), config.vad_sensitivity);

            if is_speech {
                if !*in_turn {
                    *in_turn = true;
                    buffer.clear();
                }
                *last_voice = Instant::now();
                println!("[manager] Detected speech, buffer size: {}", buffer.len());
                buffer.extend_from_slice(chunk);
            } else if *in_turn {
                buffer.extend_from_slice(chunk);
                println!("[manager] No speech detected, buffer size: {}", buffer.len());
                if last_voice.elapsed() > config.silence_threshold {
                    let audio = std::mem::take(buffer);
                    // If the stream is dropped on the other end, stop processing
                    if tx.blocking_send(audio).is_err() {
                        return Err(anyhow!("Receiver dropped"));
                    }
                    *in_turn = false;
                }
            }
        }
        Ok(())
    }
}
