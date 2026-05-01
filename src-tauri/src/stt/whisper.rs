use tokio::task;
use std::process::Command;
use std::path::PathBuf;

pub struct WhisperConfig {    
    pub model_path: String,
    pub command: String,
}

pub async fn run_transcription(config: WhisperConfig, file_path: PathBuf) -> Result<String, String> {
    // We use spawn_blocking because Command::new().output() is a synchronous, 
    // heavy CPU/IO operation that shouldn't block the async thread pool.
    println!("Running transcription with model: {}", config.model_path);
    let result = task::spawn_blocking(move || {
        let output = Command::new(&config.command)
            .arg("-m")
            .arg(&config.model_path)
            .arg("-f")
            .arg(file_path)
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            println!("Transcription command failed with status: {}", output.status);
            println!("Error output: {}", String::from_utf8_lossy(&output.stderr));
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
    println!("Raw output: {}", String::from_utf8_lossy(&output.stdout));

        let raw = String::from_utf8_lossy(&output.stdout).to_string();
        
        // Clean up the output string
        let text = raw
            .lines()
            .filter(|l| l.contains('[') && l.contains(']'))
            .map(|l| l.split(']').last().unwrap_or("").trim())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
println!("Cleaned transcription: {}", text);
        Ok(text)
    }).await;

    // Handle the JoinError from spawn_blocking, then return our Result
    match result {
        Ok(inner_result) => inner_result,
        Err(e) => Err(format!("Task panicked or failed: {}", e)),
    }
}
