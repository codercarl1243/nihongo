use std::time::Duration;

use chrono::Timelike as _;
use tauri::{AppHandle, Emitter, Manager};

use crate::events::{MicStatusEvent, PipelineStatusEvent, SessionReadyEvent, SystemMessageEvent};
use crate::AppState;
use crate::pipeline::wait_for_playback;

// TODO: utilize username from learner profile instead of hardcoded name
pub fn build_greeting() -> &'static str {
    let hour = chrono::Local::now().hour();
    match hour {
        5..=11  => "Good morning, Carl!",
        12..=16 => "Good afternoon, Carl!",
        17..=20 => "Good evening, Carl!",
        _       => "Good evening, Carl!",
    }
}

/// Speaks a hardcoded greeting via TTS directly — no LLM involved.
/// Emits `session_ready` once audio has been queued for playback.
pub async fn send_greeting(app: AppHandle) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    let greeting = build_greeting();

    app.emit("pipeline_status", PipelineStatusEvent { stage: "TTS: Synthesizing".into() }).ok(); // [PIPELINE_DEBUG]
    state.audio.lock().unwrap().mute();
    state.player.lock().unwrap().resume();

    let tts_result = async {
        let wav = state.tts.speak(greeting).await?;
        let pcm = crate::pipeline::wav_to_f32(&wav)?;
        let len = pcm.len();
        state.player.lock().unwrap().play_chunk(&pcm, 24_000)?;
        if let Some(ref sink) = *state.aec_sink.lock().unwrap() {
            sink.push(&pcm, 24_000);
        }
        // Signal that audio is now actually reaching the speaker so barge-in delay
        // is measured from this moment, not from when mute() was called (which
        // happens before TTS synthesis and would let synthesis latency eat the delay).
        state.audio.lock().unwrap().signal_audio_started();
        Ok::<usize, anyhow::Error>(len)
    }.await;

    // On TTS error: unmute immediately so the session isn't left permanently muted.
    if tts_result.is_err() {
        state.audio.lock().unwrap().unmute();
    }
    let pcm_len = tts_result?;

    app.emit("pipeline_status", PipelineStatusEvent { stage: "Audio: Playing".into() }).ok(); // [PIPELINE_DEBUG]
    let duration = Duration::from_secs_f64(pcm_len as f64 / 24_000.0);
    wait_for_playback(&state, duration).await;

    // Unmute only after audio has finished playing.
    state.audio.lock().unwrap().unmute();

    // Seed greeting into history so the LLM doesn't re-greet on the first turn.
    if let Some(ref mut s) = *state.session.lock().unwrap() {
        s.push_assistant(greeting);
    }

    app.emit("system_message", SystemMessageEvent { text: greeting.to_string() })?;
    app.emit("session_ready", SessionReadyEvent { greeting: greeting.to_string() })?;
    app.emit("mic_status", MicStatusEvent { active: true })?;
    app.emit("pipeline_status", PipelineStatusEvent { stage: "VAD 1: Listening".into() }).ok(); // [PIPELINE_DEBUG]

    Ok(())
}
