use tauri::{AppHandle, Manager};
use std::path::PathBuf;
use std::fs;


pub fn resolve_path(app: &AppHandle, path: &str) -> Option<PathBuf> {
    // 1. Use Tauri's resolver to get the official AppData directory
    // This automatically includes your bundle identifier in the path
    let mut base_path = app.path().app_data_dir().ok()?;
    
    // 2. Only push "nihongo" if you are manually creating a subfolder there
    // If your frontend saves directly to AppData, you might not even need this line
    base_path.push("voice-recordings");

    // 3. Ensure the directory exists
    if !base_path.exists() {
        let _ = fs::create_dir_all(&base_path);
    }

    Some(base_path.join(path))
}
