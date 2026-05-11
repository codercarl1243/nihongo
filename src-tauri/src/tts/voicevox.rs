use anyhow::{Context, Result};
use reqwest::Client;

/// Default speaker ID. Change after first test run by calling GET /speakers on the VoiceVox Engine.
pub const DEFAULT_SPEAKER: u32 = 3; // Zundamon ノーマル

#[derive(Clone)]
pub struct VoiceVoxClient {
    http:       Client,
    base:       String,
    speaker_id: u32,
}

impl VoiceVoxClient {
    pub fn new(speaker_id: u32) -> Self {
        Self {
            http:       Client::new(),
            base:       "http://127.0.0.1:50021".into(),
            speaker_id,
        }
    }

    /// Two-step VoiceVox synthesis: audio_query → synthesis → WAV bytes (PCM-16 LE, 24 kHz).
    pub async fn speak(&self, text: &str) -> Result<Vec<u8>> {
        let query: serde_json::Value = self.http
            .post(format!("{}/audio_query", self.base))
            .query(&[("text", text), ("speaker", &self.speaker_id.to_string())])
            .send().await.context("VoiceVox audio_query failed")?
            .error_for_status().context("VoiceVox audio_query returned error status")?
            .json().await.context("failed to parse VoiceVox audio_query response")?;

        let bytes = self.http
            .post(format!("{}/synthesis", self.base))
            .query(&[("speaker", self.speaker_id.to_string())])
            .json(&query)
            .send().await.context("VoiceVox synthesis failed")?
            .error_for_status().context("VoiceVox synthesis returned error status")?
            .bytes().await.context("failed to read VoiceVox WAV")?;

        Ok(bytes.to_vec())
    }
}
