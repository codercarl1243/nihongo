use whisper_rs::{WhisperContext, FullParams, SamplingStrategy};

pub struct VoiceEngine {
    ctx: WhisperContext,
}

impl VoiceEngine {
    pub fn new(model_path: &str) -> Self {
        let ctx = WhisperContext::new(model_path).expect("failed to load model");
        Self { ctx }
    }

    pub fn transcribe(&self, audio_data: &[f32]) -> String {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en")); // or "ja" for Nihongo!
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let mut state = self.ctx.create_state().expect("failed to create state");
        state.full(params, audio_data).expect("failed to run model");

        let num_segments = state.full_n_segments().expect("failed to get segments");
        let mut result = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                result.push_str(&segment);
            }
        }
        result
    }
}
