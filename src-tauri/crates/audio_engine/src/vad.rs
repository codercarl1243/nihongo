use anyhow::Result;
pub use voice_activity_detector::VoiceActivityDetector;

pub struct SpeechDetector {
    detector: VoiceActivityDetector,
    #[allow(dead_code)]
    sample_rate: u32,
    chunk_size: usize,
}

impl SpeechDetector {
    pub fn new(sample_rate: u32, chunk_size: usize) -> Result<Self> {
        // Silero V5 detector
        let detector = VoiceActivityDetector::builder()
            .sample_rate(sample_rate as i64)
            .chunk_size(chunk_size)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build VAD: {}", e))?;

        Ok(Self {
            detector,
            sample_rate,
            chunk_size,
        })
    }

    /// Predicts speech probability for a window of samples.
    /// Expected input size is the chunk_size (512 for 16kHz).
    pub fn is_speech(&mut self, window: Vec<f32>, threshold: f32) -> bool {
        // The crate returns a f32 probability between 0.0 and 1.0
        debug_assert_eq!(window.len(), self.chunk_size,
                        "VAD window size mismatch — expected {}", self.chunk_size);
        self.detector.predict(window) > threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 16_000;
    const CHUNK: usize     = 512;

    fn detector() -> SpeechDetector {
        SpeechDetector::new(SAMPLE_RATE, CHUNK).unwrap()
    }

    #[test]
    fn silence_is_not_speech() {
        let mut vad = detector();
        let silence = vec![0.0f32; CHUNK];
        assert!(!vad.is_speech(silence, 0.5), "silence should not be classified as speech");
    }

    #[test]
    fn is_deterministic_for_same_input() {
        // Two freshly constructed detectors (identical LSTM state) fed the same signal
        // must agree — verifies the model is stateless across instantiations.
        use std::f32::consts::PI;
        let signal: Vec<f32> = (0..CHUNK)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / SAMPLE_RATE as f32).sin())
            .collect();
        let r1 = detector().is_speech(signal.clone(), 0.5);
        let r2 = detector().is_speech(signal, 0.5);
        assert_eq!(r1, r2, "same input must produce same output on fresh detectors");
    }

    #[test]
    fn very_low_threshold_accepts_near_silence() {
        let mut vad = detector();
        // A tiny non-zero signal should cross a threshold near 0.
        let tiny: Vec<f32> = vec![0.001f32; CHUNK];
        // With threshold = 0.0 anything above 0 probability passes.
        // We don't assert a specific value — just that it doesn't panic.
        let _ = vad.is_speech(tiny, 0.0);
    }

    #[test]
    fn high_threshold_rejects_quiet_signal() {
        let mut vad = detector();
        let quiet: Vec<f32> = vec![0.01f32; CHUNK];
        // At threshold=0.99 a very quiet signal should not cross the bar.
        assert!(!vad.is_speech(quiet, 0.99));
    }
}
