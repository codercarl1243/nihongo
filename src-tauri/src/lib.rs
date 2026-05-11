use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Mutex};
use tauri::Manager;

use audio_engine::{AecSink, AudioManager, AudioPlayer};
use db::Db;
use llm::SidecarClient;
use tutor::TutorSession;

pub mod commands;
pub mod events;
pub mod greeting;
pub mod pipeline;
pub mod sidecar;
pub mod tts;

use tts::{TtsEngine, VoiceVoxClient, DEFAULT_SPEAKER};

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct AppState {
    pub audio:         Mutex<AudioManager>,
    pub player:        Mutex<AudioPlayer>,
    pub aec_sink:      Mutex<Option<AecSink>>,
    pub llm:           SidecarClient,
    pub tts:           TtsEngine,
    pub db:            Mutex<Db>,
    pub session:       Mutex<Option<TutorSession>>,
    pub sidecar_ready: AtomicBool,
}

// Compile-time path to the sidecar directory; resolves relative to src-tauri/.
pub(crate) const SIDECAR_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sidecar");

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Db::open(&db_path()).expect("failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            audio:         Mutex::new(AudioManager::new()),
            player:        Mutex::new(AudioPlayer::new()),
            aec_sink:      Mutex::new(None),
            llm:           SidecarClient::new(),
            tts:           TtsEngine::VoiceVox(VoiceVoxClient::new(DEFAULT_SPEAKER)),
            db:            Mutex::new(db),
            session:       Mutex::new(None),
            sidecar_ready: AtomicBool::new(false),
        })
        .setup(|app| {
            let sidecar_handle  = app.handle().clone();
            let voicevox_handle = app.handle().clone();
            tauri::async_runtime::spawn(sidecar::start_sidecar_background(sidecar_handle));
            tauri::async_runtime::spawn(sidecar::start_voicevox_background(voicevox_handle));

            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(|event| {
                    if let tauri::WindowEvent::Destroyed = event {
                        sidecar::shutdown();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_session,
            commands::stop_session,
            commands::barge_in,
            commands::get_sidecar_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nihongo")
        .join("nihongo.db")
}
