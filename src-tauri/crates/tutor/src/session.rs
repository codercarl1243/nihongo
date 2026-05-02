use db::{Db, LessonSummary};
use llm::{ChatMessage, TutorResponse};
use anyhow::Result;

use crate::prompt::build_system_prompt;

const CHARS_PER_TOKEN: usize = 4;
const MAX_CONTEXT_TOKENS: usize = 4096;
const COMPACT_THRESHOLD: usize = (MAX_CONTEXT_TOKENS as f64 * 0.8) as usize;

pub struct TutorSession {
    messages: Vec<ChatMessage>,
    token_estimate: usize,
    session_id: i64,
}

impl TutorSession {
    pub fn new(db: &Db) -> Result<Self> {
        let profile = db.learner_profile()?;
        let last_summary = db.latest_lesson_summary()?;
        let session_id = db.start_session()?;
        let system_msg = build_system_prompt(&profile, last_summary.as_ref());
        let estimate = token_estimate(&system_msg.content);
        Ok(Self { messages: vec![system_msg], token_estimate: estimate, session_id })
    }

    pub fn session_id(&self) -> i64 { self.session_id }

    /// Snapshot of the full message history for streaming against the LLM.
    pub fn current_messages(&self) -> &[ChatMessage] { &self.messages }

    /// Append the student's transcript as a user message.
    pub fn push_user(&mut self, transcript: &str) {
        self.messages.push(ChatMessage::user(transcript));
        self.token_estimate += token_estimate(transcript);
    }

    /// Synchronously persist the completed turn and append the assistant message.
    /// Returns `true` when the context has crossed the compaction threshold.
    pub fn record_turn_sync(
        &mut self,
        transcript: &str,
        response: &TutorResponse,
        db: &Db,
    ) -> Result<bool> {
        self.messages.push(ChatMessage::assistant(&response.response));
        self.token_estimate += token_estimate(&response.response);

        db.record_turn(
            self.session_id,
            transcript,
            &response.response,
            response.milestone,
        )?;

        Ok(response.milestone && self.token_estimate >= COMPACT_THRESHOLD)
    }

    /// Build a summary-request message list from the current history.
    /// Called by lib.rs before streaming the summary.
    pub fn summary_request_messages(&self) -> Vec<ChatMessage> {
        let history: String = self.messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        vec![
            ChatMessage::system("You are a concise session summariser."),
            ChatMessage::user(format!(
                "Summarise this tutoring session in JSON:\n\
                 {{\"topics_covered\": [...], \"words_introduced\": [], \"words_reviewed\": [], \"continue_from\": \"...\"}}\n\n\
                 Session:\n{}",
                history
            )),
        ]
    }

    /// Reset the context window with a new system prompt after compaction.
    pub fn reset_context(&mut self, system_msg: ChatMessage) {
        self.token_estimate = token_estimate(&system_msg.content);
        self.messages = vec![system_msg];
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers — pub so lib.rs can use them
// ---------------------------------------------------------------------------

pub fn parse_response_pub(raw: &str) -> TutorResponse {
    let trimmed = raw.trim();
    if let (Some(s), Some(e)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if let Ok(r) = serde_json::from_str::<TutorResponse>(&trimmed[s..=e]) {
            return r;
        }
    }
    TutorResponse { transcript: String::new(), response: raw.to_string(), milestone: false }
}

pub fn parse_summary_pub(raw: &str) -> LessonSummary {
    #[derive(serde::Deserialize)]
    struct Raw { topics_covered: Vec<String>, continue_from: String }
    let trimmed = raw.trim();
    if let (Some(s), Some(e)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if let Ok(r) = serde_json::from_str::<Raw>(&trimmed[s..=e]) {
            return LessonSummary {
                id: 0,
                topics_covered: r.topics_covered,
                words_introduced: vec![],
                words_reviewed: vec![],
                continue_from: r.continue_from,
            };
        }
    }
    LessonSummary {
        id: 0,
        topics_covered: vec![],
        words_introduced: vec![],
        words_reviewed: vec![],
        continue_from: raw.chars().take(200).collect(),
    }
}

fn token_estimate(text: &str) -> usize {
    text.len().saturating_div(CHARS_PER_TOKEN).max(1)
}
