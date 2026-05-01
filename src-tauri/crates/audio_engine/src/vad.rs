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
