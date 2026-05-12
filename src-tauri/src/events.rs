use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct SidecarStatusEvent  { pub state: String, pub message: Option<String> }

#[derive(Clone, Serialize)]
pub struct TtsStatusEvent {
    pub state:    String,       // "checking" | "downloading" | "extracting" | "starting" | "ready" | "error"
    pub progress: Option<f32>,  // 0.0–1.0 during "downloading"
    pub message:  Option<String>,
}

#[derive(Clone, Serialize)]
pub struct TranscriptEvent     { pub text: String }

#[derive(Clone, Serialize)]
pub struct ResponseDoneEvent   { pub full_response: String, pub milestone: bool, pub prompt_tokens: u32 }

#[derive(Clone, Serialize)]
pub struct SessionReadyEvent   { pub greeting: String }

#[derive(Clone, Serialize)]
pub struct ErrorEvent          { pub message: String }

#[derive(Clone, Serialize)]
pub struct MicStatusEvent      { pub active: bool }

#[derive(Clone, Serialize)]
pub struct PipelineStatusEvent { pub stage: String }   // [PIPELINE_DEBUG]

#[derive(Clone, Serialize)]
pub struct SystemMessageEvent  { pub text: String }
