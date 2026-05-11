pub mod qwen3;
pub mod voicevox;

pub use qwen3::Qwen3TtsClient;
pub use voicevox::{VoiceVoxClient, DEFAULT_SPEAKER};

use anyhow::Result;

#[derive(Clone)]
pub enum TtsEngine {
    Qwen3(Qwen3TtsClient),
    VoiceVox(VoiceVoxClient),
}

impl TtsEngine {
    pub async fn speak(&self, text: &str) -> Result<Vec<u8>> {
        match self {
            Self::Qwen3(c)    => c.speak(text).await,
            Self::VoiceVox(c) => c.speak(text).await,
        }
    }
}
