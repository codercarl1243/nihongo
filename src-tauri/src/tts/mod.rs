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

    /// Returns true when this engine's backing service is confirmed healthy.
    /// Only the active variant is consulted — unused providers are ignored.
    pub fn is_ready(&self, sidecar_ready: bool, tts_ready: bool) -> bool {
        match self {
            Self::VoiceVox(_) => tts_ready,
            Self::Qwen3(_)    => sidecar_ready,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voicevox_gates_on_voicevox_flag() {
        let engine = TtsEngine::VoiceVox(VoiceVoxClient::new(DEFAULT_SPEAKER));
        assert!(!engine.is_ready(true,  false), "VoiceVox not ready → false even if sidecar is up");
        assert!( engine.is_ready(false, true),  "VoiceVox ready → true even if sidecar is down");
        assert!( engine.is_ready(true,  true),  "both ready → true");
    }

    #[test]
    fn qwen3_gates_on_sidecar_flag() {
        let engine = TtsEngine::Qwen3(Qwen3TtsClient::new("http://127.0.0.1:8091"));
        assert!(!engine.is_ready(false, true),  "sidecar down → false even if voicevox is up");
        assert!( engine.is_ready(true,  false), "sidecar ready → true even if voicevox is down");
        assert!( engine.is_ready(true,  true),  "both ready → true");
    }
}
