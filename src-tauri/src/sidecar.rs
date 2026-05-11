use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio_stream::StreamExt as _;

use crate::events::{SidecarStatusEvent, VoiceVoxStatusEvent};
use crate::AppState;
use crate::SIDECAR_DIR;

// ---------------------------------------------------------------------------
// Managed child processes — all processes killed on exit live here.
// Add new entries as needed; `shutdown()` iterates them all.
// ---------------------------------------------------------------------------

static SIDECAR_CHILD:  Mutex<Option<std::process::Child>> = Mutex::new(None);
static VOICEVOX_CHILD: Mutex<Option<std::process::Child>> = Mutex::new(None);

/// Kill all managed background processes. Called once when the app window closes.
pub fn shutdown() {
    kill_child(&SIDECAR_CHILD);
    kill_child(&VOICEVOX_CHILD);
}

fn kill_child(slot: &Mutex<Option<std::process::Child>>) {
    if let Some(mut child) = slot.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

// ---------------------------------------------------------------------------
// Python sidecar
// ---------------------------------------------------------------------------

async fn sidecar_is_up() -> bool {
    reqwest::get("http://127.0.0.1:8091/health")
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Spawned at startup. Starts the sidecar if not already running, then polls
/// until ready. Emits `sidecar_status` events for the frontend loading state.
/// The sidecar is intentionally left running when the app exits so models
/// stay warm in GPU memory between sessions.
pub async fn start_sidecar_background(app: AppHandle) {
    let emit = |state: &str, message: Option<String>| {
        let _ = app.emit("sidecar_status", SidecarStatusEvent {
            state: state.to_string(),
            message,
        });
    };

    emit("warming_up", None);

    let mark_ready = || {
        use std::sync::atomic::Ordering;
        app.state::<AppState>().sidecar_ready.store(true, Ordering::SeqCst);
        let _ = app.emit("sidecar_status", SidecarStatusEvent {
            state: "ready".into(),
            message: None,
        });
    };

    if sidecar_is_up().await {
        mark_ready();
        return;
    }

    // Spawn start.sh. stdout/stderr inherit so log output stays visible in
    // the terminal where the app was launched. The handle is stored so
    // shutdown() can kill the process (start.sh uses `exec uvicorn`, so the
    // stored PID is uvicorn's PID once the script hands off).
    match std::process::Command::new("bash")
        .arg("start.sh")
        .current_dir(SIDECAR_DIR)
        .spawn()
    {
        Ok(child) => { *SIDECAR_CHILD.lock().unwrap() = Some(child); }
        Err(e)    => { emit("error", Some(format!("Failed to launch sidecar: {e}"))); return; }
    }

    // Poll every 2 s for up to 2 minutes.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if sidecar_is_up().await {
            mark_ready();
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            emit("error", Some("Sidecar did not become ready within 2 minutes".into()));
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// VoiceVox Engine
// ---------------------------------------------------------------------------

const VOICEVOX_VERSION: &str = "0.25.2";
const VOICEVOX_HOST: &str = "127.0.0.1";
const VOICEVOX_PORT: u16 = 50021;

fn voicevox_engine_dir() -> std::path::PathBuf {
    std::path::Path::new(SIDECAR_DIR).join("voicevox_engine")
}

async fn voicevox_is_up() -> bool {
    reqwest::get(format!("http://{VOICEVOX_HOST}:{VOICEVOX_PORT}/version"))
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Emits `voicevox_status` events. States:
///   "checking" → "downloading" (with progress 0.0–1.0) → "extracting" → "starting" → "ready" | "error"
pub async fn start_voicevox_background(app: AppHandle) {
    if let Err(e) = run_voicevox_background(&app).await {
        let _ = app.emit("voicevox_status", VoiceVoxStatusEvent {
            state:    "error".into(),
            progress: None,
            message:  Some(e.to_string()),
        });
    }
}

async fn run_voicevox_background(app: &AppHandle) -> anyhow::Result<()> {
    let emit = |state: &str, progress: Option<f32>, message: Option<&str>| {
        let _ = app.emit("voicevox_status", VoiceVoxStatusEvent {
            state:    state.to_string(),
            progress,
            message:  message.map(str::to_string),
        });
    };

    emit("checking", None, None);

    // Already running (e.g. leftover from previous launch).
    if voicevox_is_up().await {
        emit("ready", None, None);
        return Ok(());
    }

    let engine_dir = voicevox_engine_dir();

    if !engine_dir.exists() {
        download_and_extract(app, &engine_dir).await?;
    }

    // Start the engine binary.
    emit("starting", None, Some("Starting VoiceVox Engine…"));
    let run_bin = engine_dir.join("run");
    let child = std::process::Command::new(&run_bin)
        .args(["--host", VOICEVOX_HOST, "--port", &VOICEVOX_PORT.to_string()])
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn VoiceVox Engine at {}: {e}", run_bin.display()))?;

    // Store the handle so shutdown() can kill it cleanly on exit.
    *VOICEVOX_CHILD.lock().unwrap() = Some(child);

    // Poll until healthy (up to 60 s).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if voicevox_is_up().await {
            emit("ready", None, None);
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("VoiceVox Engine did not become ready within 60 seconds");
        }
    }
}

async fn download_and_extract(
    app: &AppHandle,
    engine_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let emit = |state: &str, progress: Option<f32>, message: Option<&str>| {
        let _ = app.emit("voicevox_status", VoiceVoxStatusEvent {
            state:    state.to_string(),
            progress,
            message:  message.map(str::to_string),
        });
    };

    let archive_name = format!("voicevox_engine-macos-arm64-{VOICEVOX_VERSION}.7z.001");
    let url = format!(
        "https://github.com/VOICEVOX/voicevox_engine/releases/download/{VOICEVOX_VERSION}/{archive_name}"
    );
    let tmp_path = std::env::temp_dir().join(&archive_name);

    emit("downloading", Some(0.0), Some("Downloading VoiceVox Engine…"));

    // Stream download with progress reporting.
    let response = reqwest::get(&url).await
        .map_err(|e| anyhow::anyhow!("download request failed: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("download returned error status: {e}"))?;

    let content_length = response.content_length();
    let mut received: u64 = 0;
    let mut file = tokio::fs::File::create(&tmp_path).await
        .map_err(|e| anyhow::anyhow!("failed to create temp file: {e}"))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("download stream error: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await
            .map_err(|e| anyhow::anyhow!("failed to write archive chunk: {e}"))?;
        received += chunk.len() as u64;
        if let Some(total) = content_length {
            emit("downloading", Some(received as f32 / total as f32), Some("Downloading VoiceVox Engine…"));
        }
    }
    drop(file);

    emit("extracting", None, Some("Extracting VoiceVox Engine…"));

    // Use 7zz (p7zip) rather than a pure-Rust library: it correctly restores
    // symlinks and Unix permissions that sevenz-rust2 silently drops.
    let status = tokio::process::Command::new("7zz")
        .args(["x", "-y", tmp_path.to_str().unwrap(), &format!("-o{SIDECAR_DIR}")])
        .status().await
        .map_err(|e| anyhow::anyhow!("failed to run 7zz: {e}"))?;
    if !status.success() {
        anyhow::bail!("7zz extraction failed with status {status}");
    }

    // The archive extracts to `macos-arm64/`. Rename to the stable `voicevox_engine/` path.
    let extracted_candidate = std::path::Path::new(SIDECAR_DIR).join("macos-arm64");
    if extracted_candidate.exists() {
        std::fs::rename(&extracted_candidate, engine_dir)
            .map_err(|e| anyhow::anyhow!("failed to rename extracted directory: {e}"))?;
    }

    // Ensure the main binary is executable (archive may not preserve the bit).
    let run_bin = engine_dir.join("run");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&run_bin)
            .map_err(|e| anyhow::anyhow!("failed to read run binary metadata: {e}"))?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&run_bin, perms)
            .map_err(|e| anyhow::anyhow!("failed to chmod run binary: {e}"))?;
    }

    let _ = tokio::fs::remove_file(&tmp_path).await;

    Ok(())
}
