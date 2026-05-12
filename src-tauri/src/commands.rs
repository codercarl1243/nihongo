use std::sync::Arc;

use audio_engine::EngineConfig;
use tauri::{AppHandle, Emitter, State};
use tokio_stream::StreamExt;
use tutor::{Japanese, LangConfig, TutorSession};

use crate::events::{ErrorEvent, PipelineStatusEvent};
use crate::AppState;
use crate::greeting::send_greeting;
use crate::pipeline::handle_turn;

#[tauri::command]
pub async fn start_session(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if !state.llm.is_ready().await {
        return Err(
            "Sidecar is still loading — please wait for models to initialise.".to_string()
        );
    }

    let lang: LangConfig = Arc::new(Japanese);

    // Silence threshold scales with proficiency: beginners need more time to
    // retrieve words; near-native speakers can pace like natural conversation.
    let silence_ms = {
        let db = state.db.lock().unwrap();
        let level = db.session_context().map(|c| c.profile.current_level).unwrap_or(5);
        lang.silence_threshold_ms(level)
    };

    {
        let audio = state.audio.lock().unwrap();
        audio.init_silence_threshold(silence_ms);
    }

    let streams = state.audio
        .lock().unwrap()
        .start(EngineConfig::default())
        .map_err(|e| e.to_string())?;

    *state.aec_sink.lock().unwrap() = Some(streams.aec_sink);

    state.player.lock().unwrap().start().map_err(|e| e.to_string())?;

    {
        let db = state.db.lock().unwrap();
        let session = TutorSession::new(&db, lang).map_err(|e| e.to_string())?;
        *state.session.lock().unwrap() = Some(session);
    }

    let llm          = state.llm.clone();
    let tts          = state.tts.clone();
    let mut turn_stream    = streams.turn_end;
    let mut partial_stream = streams.partial;
    let mut vad_stream     = streams.vad_state; // [PIPELINE_DEBUG]
    let greeting_app = app.clone();

    // Keep the partial receiver alive; drain without consuming.
    tokio::spawn(async move {
        while partial_stream.next().await.is_some() {}
    });

    // [PIPELINE_DEBUG] — remove this block and the vad_state stream when done
    let vad_status_app = app.clone();
    tokio::spawn(async move {
        while let Some(stage) = vad_stream.next().await {
            let _ = vad_status_app.emit(
                "pipeline_status",
                PipelineStatusEvent { stage: stage.to_string() },
            );
        }
    });

    tokio::spawn(async move {
        while let Some(audio_chunk) = turn_stream.next().await {
            if let Err(e) = handle_turn(audio_chunk, &app, &llm, &tts).await {
                let _ = app.emit("error", ErrorEvent { message: e.to_string() });
            }
        }
    });

    tokio::spawn(async move {
        if let Err(e) = send_greeting(greeting_app.clone()).await {
            let _ = greeting_app.emit("error", ErrorEvent { message: e.to_string() });
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_session(state: State<'_, AppState>) -> Result<(), String> {
    state.audio.lock().unwrap().stop().map_err(|e| e.to_string())?;
    state.player.lock().unwrap().stop();
    if let Some(s) = state.session.lock().unwrap().take() {
        state.db.lock().unwrap()
            .end_session(s.session_id())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn barge_in(state: State<'_, AppState>) {
    state.player.lock().unwrap().stop();
}

#[tauri::command]
pub fn get_sidecar_ready(state: State<'_, AppState>) -> bool {
    use std::sync::atomic::Ordering;
    let sidecar_ready  = state.sidecar_ready.load(Ordering::SeqCst);
    let voicevox_ready = state.voicevox_ready.load(Ordering::SeqCst);
    sidecar_ready && state.tts.is_ready(sidecar_ready, voicevox_ready)
}
