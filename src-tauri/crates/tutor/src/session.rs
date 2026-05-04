use db::{Db, LessonSummary, TopicStatus};
use llm::{ChatMessage, TutorResponse};
use anyhow::Result;

use crate::prompt::build_system_prompt;

const CHARS_PER_TOKEN: usize = 4;
const MAX_CONTEXT_TOKENS: usize = 4096;
const COMPACT_THRESHOLD: usize = (MAX_CONTEXT_TOKENS as f64 * 0.8) as usize;

pub struct TutorSession {
    messages:       Vec<ChatMessage>,
    token_estimate: usize,
    session_id:     i64,
}

impl TutorSession {
    pub fn new(db: &Db) -> Result<Self> {
        let ctx = db.session_context()?;

        // Ensure the active topic is recorded as in_progress.
        if let Some(ref active) = ctx.current_topic {
            db.mark_topic_status(active.topic.id, TopicStatus::InProgress)?;
        }

        let session_id = db.start_session()?;
        let system_msg = build_system_prompt(&ctx);
        let estimate   = token_estimate(&system_msg.content);
        Ok(Self { messages: vec![system_msg], token_estimate: estimate, session_id })
    }

    pub fn session_id(&self) -> i64 { self.session_id }

    pub fn current_messages(&self) -> &[ChatMessage] { &self.messages }

    pub fn push_user(&mut self, transcript: &str) {
        self.messages.push(ChatMessage::user(transcript));
        self.token_estimate += token_estimate(transcript);
    }

    /// Inject an assistant message without recording a DB turn.
    /// Used to seed the greeting into history so the model doesn't re-greet.
    pub fn push_assistant(&mut self, text: &str) {
        self.messages.push(ChatMessage::assistant(text));
        self.token_estimate += token_estimate(text);
    }

    /// Persist the completed turn and append the assistant message.
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
                 {{\"notes\": \"...\"}}\n\n\
                 The notes field should describe what was practised, any errors made, \
                 and what to continue next session. Be specific about vocabulary and topics.\n\n\
                 Session:\n{}",
                history
            )),
        ]
    }

    pub fn reset_context(&mut self, system_msg: ChatMessage) {
        self.token_estimate = token_estimate(&system_msg.content);
        self.messages = vec![system_msg];
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

pub fn parse_response_pub(raw: &str) -> TutorResponse {
    // TODO: detect milestones (student answered correctly) to trigger context compaction
    TutorResponse { transcript: String::new(), response: raw.trim().to_string(), milestone: false }
}

pub fn parse_summary_pub(raw: &str) -> LessonSummary {
    #[derive(serde::Deserialize)]
    struct Raw { notes: String }
    let trimmed = raw.trim();
    if let (Some(s), Some(e)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if let Ok(r) = serde_json::from_str::<Raw>(&trimmed[s..=e]) {
            return LessonSummary {
                id: 0, session_id: None, notes: r.notes,
                topics: vec![], vocabulary: vec![],
            };
        }
    }
    LessonSummary {
        id: 0,
        session_id: None,
        notes: raw.chars().take(500).collect(),
        topics: vec![],
        vocabulary: vec![],
    }
}

fn token_estimate(text: &str) -> usize {
    text.len().saturating_div(CHARS_PER_TOKEN).max(1)
}
