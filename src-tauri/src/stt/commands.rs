use crate::stt::transcribe::transcribe_audio;
use tauri::{AppHandle};

#[tauri::command]
pub async fn transcribe_audio_cmd(app: AppHandle, path: String) -> Result<String, String> {
    transcribe_audio(app, path).await
}
