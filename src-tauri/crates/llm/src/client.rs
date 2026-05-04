use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_stream::{Stream, StreamExt};

const SIDECAR_URL: &str = "http://127.0.0.1:8091";

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

/// Parsed from every SSE token emitted by /llm/chat.
#[derive(Debug, Clone, Deserialize)]
pub struct TutorToken {
    pub token: String,
}

/// The complete structured response assembled from the token stream.
#[derive(Debug, Clone, Deserialize)]
pub struct TutorResponse {
    pub transcript: String,
    pub response: String,
    pub milestone: bool,
}

/// Actual token counts reported by the sidecar at end of stream.
#[derive(Debug, Clone, Default)]
pub struct ChatUsage {
    pub prompt_tokens:     u32,
    pub generation_tokens: u32,
}

/// Items emitted by `chat_stream`.
pub enum StreamItem {
    Token(String),
    Usage(ChatUsage),
}

#[derive(Deserialize)]
struct UsagePayload { prompt_tokens: u32, generation_tokens: u32 }
#[derive(Deserialize)]
struct UsageEvent   { usage: UsagePayload }

// ---------------------------------------------------------------------------
// SidecarClient
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SidecarClient {
    http: Client,
    base: String,
}

impl SidecarClient {
    pub fn new() -> Self {
        Self {
            http: Client::new(),
            base: SIDECAR_URL.to_string(),
        }
    }

    pub fn with_url(url: impl Into<String>) -> Self {
        Self { http: Client::new(), base: url.into() }
    }

    /// Send a 16kHz mono f32 audio clip to the ASR endpoint.
    /// `language` is a hard decoder constraint (e.g. "en", "ja") that prevents
    /// drift to unrelated languages like Hindi or Cantonese.
    pub async fn transcribe(&self, audio: &[f32], language: Option<&str>) -> Result<String> {
        let raw: Vec<u8> = audio
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let audio_b64 = base64_encode(&raw);

        #[derive(Serialize)]
        struct Req<'a> {
            audio_b64: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            language: Option<&'a str>,
        }
        #[derive(Deserialize)]
        struct Resp { transcript: String }

        let resp: Resp = self.http
            .post(format!("{}/asr/transcribe", self.base))
            .json(&Req { audio_b64, language })
            .send()
            .await
            .context("ASR request failed")?
            .error_for_status()
            .context("ASR returned error status")?
            .json()
            .await
            .context("failed to parse ASR response")?;

        Ok(resp.transcript)
    }

    /// Send a conversation history to the LLM and stream items back.
    /// Each item is either a `Token(String)` or a final `Usage(ChatUsage)`.
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<impl Stream<Item = Result<StreamItem>>> {
        #[derive(Serialize)]
        struct Req<'a> { messages: &'a [ChatMessage] }

        let resp = self.http
            .post(format!("{}/llm/chat", self.base))
            .json(&Req { messages })
            .send()
            .await
            .context("LLM chat request failed")?
            .error_for_status()
            .context("LLM returned error status")?;

        let byte_stream = resp.bytes_stream();

        let stream = byte_stream.map(|chunk| -> Result<StreamItem> {
            let bytes = chunk.context("stream read error")?;
            let text = std::str::from_utf8(&bytes).context("invalid UTF-8 in stream")?;

            // SSE lines: `data: {"token": "..."}`, `data: {"usage": {...}}`, `data: [DONE]`
            let mut tokens = String::new();
            for line in text.lines() {
                let Some(data) = line.strip_prefix("data: ") else { continue };
                if data == "[DONE]" { break; }
                if let Ok(u) = serde_json::from_str::<UsageEvent>(data) {
                    return Ok(StreamItem::Usage(ChatUsage {
                        prompt_tokens:     u.usage.prompt_tokens,
                        generation_tokens: u.usage.generation_tokens,
                    }));
                }
                if let Ok(t) = serde_json::from_str::<TutorToken>(data) {
                    tokens.push_str(&t.token);
                }
            }
            Ok(StreamItem::Token(tokens))
        });

        Ok(stream)
    }

    /// Send text to the TTS endpoint, receive WAV bytes back.
    pub async fn speak(&self, text: &str) -> Result<Vec<u8>> {
        #[derive(Serialize)]
        struct Req<'a> { text: &'a str }

        let bytes = self.http
            .post(format!("{}/tts/speak", self.base))
            .json(&Req { text })
            .send()
            .await
            .context("TTS request failed")?
            .error_for_status()
            .context("TTS returned error status")?
            .bytes()
            .await
            .context("failed to read TTS audio")?;

        Ok(bytes.to_vec())
    }

    /// Single-shot health ping — returns true if the sidecar is up right now.
    pub async fn is_ready(&self) -> bool {
        self.http
            .get(format!("{}/health", self.base))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Wait until the sidecar is ready (retries for up to `timeout_secs`).
    pub async fn wait_until_ready(&self, timeout_secs: u64) -> Result<()> {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(timeout_secs);

        loop {
            if self.http
                .get(format!("{}/health", self.base))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
            {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                anyhow::bail!("sidecar did not become ready within {timeout_secs}s");
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}

impl Default for SidecarClient {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn base64_encode(data: &[u8]) -> String {
    // Use the standard base64 alphabet without padding differences
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = Vec::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        out.push(TABLE[(b0 >> 2) & 0x3f]);
        out.push(TABLE[((b0 << 4) | (b1 >> 4)) & 0x3f]);
        out.push(if chunk.len() > 1 { TABLE[((b1 << 2) | (b2 >> 6)) & 0x3f] } else { b'=' });
        out.push(if chunk.len() > 2 { TABLE[b2 & 0x3f] } else { b'=' });
    }
    String::from_utf8(out).unwrap()
}
