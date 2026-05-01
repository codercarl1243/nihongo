use tauri::{AppHandle};
use crate::files::path::resolve_path;
use super::whisper::{run_transcription, WhisperConfig};

pub async fn transcribe_audio(app: AppHandle, path: String) -> Result<String, String> {
    println!("1");
    
    let full_path = resolve_path(&app, &path).ok_or("Failed to resolve path")?;
    
 println!("2: Resolved path: {:?}", full_path);
    let config = WhisperConfig {
        model_path: "/Users/carl/llm_models/ggml-base.bin".to_string(),
        command: "whisper-cli".to_string(),
    };
 println!("3: Config initialized");
    run_transcription(config, full_path).await

}
