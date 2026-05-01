use anyhow::{anyhow, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// A single transcribed segment with its text and timestamps.
/// Timestamps are in centiseconds (multiply by 10 to get milliseconds).
#[derive(Debug, Clone)]
pub struct Segment {
    pub text: String,
    pub start_cs: i64,
    pub end_cs: i64,
}

impl Segment {
    pub fn start_ms(&self) -> i64 { self.start_cs * 10 }
    pub fn end_ms(&self) -> i64   { self.end_cs   * 10 }
}

/// Configuration for the Whisper transcriber.
pub struct TranscriberConfig {
    /// Path to the GGML model file e.g. ggml-large-v3-turbo-q5_0.bin
    pub model_path: String,
    /// BCP-47 language code. "auto" lets Whisper detect — costs ~200ms extra.
    pub language: String,
    /// Translate output to English regardless of source language.
    pub translate: bool,
    /// Primes the decoder — use to hint mixed EN/JA content.
    pub initial_prompt: Option<String>,
}

impl TranscriberConfig {
    pub fn new(model_path: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            model_path: model_path.into(),
            language: language.into(),
            translate: false,
            initial_prompt: None,
        }
    }
}

/// Wraps a loaded Whisper model. Load once at startup, call transcribe() per turn.
///
/// # Thread safety
/// `WhisperContext` is not `Send` — keep `Transcriber` on a single dedicated thread
/// and communicate via channels (see integration.rs for the pattern).
pub struct Transcriber {
    ctx: WhisperContext,
    config: TranscriberConfig,
}

impl Transcriber {
    pub fn new(config: TranscriberConfig) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(
            &config.model_path,
            WhisperContextParameters::default(),
        )
        .map_err(|e| anyhow!("Failed to load model '{}': {:?}", config.model_path, e))?;

        println!("[transcriber] model loaded: {}", config.model_path);
        Ok(Self { ctx, config })
    }

    /// Transcribe 16kHz mono f32 audio. Returns all segments found.
    pub fn transcribe(&self, audio: &[f32]) -> Result<Vec<Segment>> {
        if audio.is_empty() {
            return Ok(vec![]);
        }

        let mut state = self.ctx
            .create_state()
            .map_err(|e| anyhow!("Failed to create Whisper state: {:?}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        if self.config.language != "auto" {
            params.set_language(Some(&self.config.language));
        }

        params.set_translate(self.config.translate);
        params.set_suppress_nst(true);       // suppress non-speech tokens
        params.set_n_threads(1);
        params.set_token_timestamps(false);

        if let Some(prompt) = &self.config.initial_prompt {
            params.set_initial_prompt(prompt);
        }

        state
            .full(params, audio)
            .map_err(|e| anyhow!("Whisper inference failed: {:?}", e))?;

        // as_iter() yields WhisperSegment — Display gives the text,
        // start/end_timestamp() give centiseconds.
        let segments = state
            .as_iter()
            .map(|seg| {
                let text = seg.to_string().trim().to_string();
                Segment {
                    text,
                    start_cs: seg.start_timestamp(),
                    end_cs: seg.end_timestamp(),
                }
            })
            .filter(|seg| !seg.text.is_empty())
            .collect();

        Ok(segments)
    }

    /// Convenience — joins all segment text into a single string.
    pub fn transcribe_to_text(&self, audio: &[f32]) -> Result<String> {
        Ok(self
            .transcribe(audio)?
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" "))
    }
}