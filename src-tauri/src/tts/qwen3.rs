use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;

#[derive(Clone)]
pub struct Qwen3TtsClient {
    http: Client,
    base: String,
}

impl Qwen3TtsClient {
    pub fn new(sidecar_url: impl Into<String>) -> Self {
        Self { http: Client::new(), base: sidecar_url.into() }
    }

    pub async fn speak(&self, text: &str) -> Result<Vec<u8>> {
        #[derive(serde::Serialize)]
        struct Req<'a> { text: &'a str }

        let bytes = self.http
            .post(format!("{}/tts/speak", self.base))
            .timeout(Duration::from_secs(60))
            .json(&Req { text })
            .send().await.context("Qwen3-TTS request failed")?
            .error_for_status().context("Qwen3-TTS returned error status")?
            .bytes().await.context("failed to read Qwen3-TTS audio")?;

        Ok(bytes.to_vec())
    }
}
