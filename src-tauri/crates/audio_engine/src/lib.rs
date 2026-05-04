//! # Audio Engine
//!
//! `audio_engine` is a real-time speech segmentation library designed for high-accuracy 
//! voice applications like language learning.
//!
//! It standardizes system microphone input into a consistent **16kHz mono f32** stream 
//! and uses neural-network-based Voice Activity Detection (VAD) to identify when a user 
//! starts and stops speaking.
//!
//! ## Core Components
//! * **Capture**: Resilient hardware management via `cpal`.
//! * **Resampler**: High-fidelity FFT-based resampling via `rubato`.
//! * **Speech Detection**: Industrial-grade VAD via `voice_activity_detector`.
//!
//! ## Basic Example
//!
//! ```rust,no_run
//! use audio_engine::{AudioManager, EngineConfig};
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let manager = AudioManager::new();
//!     let mut streams = manager.start(EngineConfig::default())?;
//!
//!     while let Some(audio_data) = streams.turn_end.next().await {
//!         println!("Speech turn captured: {} samples", audio_data.len());
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Tuning for Language Learners
//! For students who may hesitate or speak softly, you can adjust the sensitivity:
//!
//! ```rust
//! use audio_engine::EngineConfig;
//! use std::time::Duration;
//!
//! let student_config = EngineConfig {
//!     silence_threshold: Duration::from_millis(1000), // Wait longer before ending turn
//!     vad_sensitivity: 0.4,                           // More sensitive to quiet speech
//!     ..Default::default()
//! };
//! ```
//! default configuration is optimized for general use:
//!   * **vad_window**: 512,
//!   * **silence_threshold**: Duration::from_millis(600),
//!   * **vad_sensitivity**: 0.5,


pub mod capture;
pub mod player;
pub mod resampler;
pub mod vad;
pub mod manager;

pub use manager::{AudioManager, EngineConfig};
pub use capture::{AudioCapture, CaptureConfig};
pub use player::AudioPlayer;
pub use resampler::{Resampler16k, TARGET_SAMPLE_RATE};
pub use vad::SpeechDetector;