//! # Transcriber
//!
//! Receives a `Vec<f32>` of 16kHz mono audio and returns transcript text.
//! This crate knows nothing about microphones, ring buffers, or VAD.
//! It has one job: audio in, text out.

pub mod whisper;

pub use whisper::{Transcriber, TranscriberConfig, Segment};