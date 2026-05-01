pub mod transcriber;
pub mod vad;
 
// Re-export the types callers will actually use so Tauri commands
// only need to import from `audio_engine`, not the submodules.
pub use transcriber::{Transcriber, TranscriptionConfig};
pub use vad::{VAD, VADConfig};
 