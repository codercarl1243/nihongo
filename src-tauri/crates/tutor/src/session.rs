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

/// Build a `TutorResponse` from the raw LLM text and pre-resolved classification flags.
/// Classification is performed by a separate `SidecarClient::classify_turn` call in the
/// main pipeline so this function stays pure and fast.
pub fn parse_response_pub(raw: &str, milestone: bool, correction: bool) -> TutorResponse {
    TutorResponse {
        transcript: String::new(),
        response:   raw.trim().to_string(),
        milestone,
        correction,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_summary_pub ────────────────────────────────────────────────────

    #[test]
    fn parse_summary_extracts_notes_from_valid_json() {
        let raw = r#"{"notes": "Student practised greetings."}"#;
        let summary = parse_summary_pub(raw);
        assert_eq!(summary.notes, "Student practised greetings.");
    }

    #[test]
    fn parse_summary_handles_json_with_surrounding_text() {
        let raw = "Here is the summary:\n{\"notes\": \"Worked on て-form.\"}\nEnd.";
        let summary = parse_summary_pub(raw);
        assert_eq!(summary.notes, "Worked on て-form.");
    }

    #[test]
    fn parse_summary_falls_back_to_raw_text_on_invalid_json() {
        let raw = "No JSON here at all.";
        let summary = parse_summary_pub(raw);
        assert_eq!(summary.notes, "No JSON here at all.");
    }

    #[test]
    fn parse_summary_truncates_long_fallback_to_500_chars() {
        let raw = "x".repeat(600);
        let summary = parse_summary_pub(&raw);
        assert_eq!(summary.notes.len(), 500);
    }

    // ── parse_response_pub ───────────────────────────────────────────────────

    #[test]
    fn parse_response_trims_whitespace() {
        let resp = parse_response_pub("  こんにちは！  ", false, false);
        assert_eq!(resp.response, "こんにちは！");
    }

    #[test]
    fn parse_response_preserves_flags() {
        let resp = parse_response_pub("Well done!", true, false);
        assert!(resp.milestone);
        assert!(!resp.correction);

        let resp2 = parse_response_pub("Not quite.", false, true);
        assert!(!resp2.milestone);
        assert!(resp2.correction);
    }

    // ── token_estimate ───────────────────────────────────────────────────────

    #[test]
    fn token_estimate_empty_string_returns_one() {
        assert_eq!(token_estimate(""), 1);
    }

    #[test]
    fn token_estimate_four_chars_is_one_token() {
        assert_eq!(token_estimate("abcd"), 1);
    }

    #[test]
    fn token_estimate_scales_with_length() {
        assert_eq!(token_estimate("a".repeat(400).as_str()), 100);
    }
}
