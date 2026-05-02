use db::{LearnerProfile, LessonSummary};
use llm::ChatMessage;

const SYSTEM_BASE: &str = "\
You are a friendly Japanese language tutor. Your student is a native English speaker.

Rules:
- Always respond in the student's current JLPT level or below.
- When the student asks about a Japanese word or phrase, explain it clearly in English, \
  then use it naturally in your Japanese response.
- Keep responses concise — one teaching point per turn.
- Always end your response with natural spoken Japanese the student can repeat.

IMPORTANT — you must respond with a JSON object only, no other text:
{
  \"transcript\": \"<exactly what the student said>\",
  \"response\": \"<your tutor response>\",
  \"milestone\": <true if the student answered correctly and you gave positive feedback, otherwise false>
}";

pub fn build_system_prompt_pub(
    profile: &LearnerProfile,
    last_summary: Option<&LessonSummary>,
) -> ChatMessage {
    build_system_prompt(profile, last_summary)
}

pub fn build_system_prompt(
    profile: &LearnerProfile,
    last_summary: Option<&LessonSummary>,
) -> ChatMessage {
    let level_desc = match profile.current_level {
        5 => "N5 (absolute beginner — hiragana, katakana, ~100 basic words)",
        4 => "N4 (elementary — ~300 words, basic grammar)",
        3 => "N3 (intermediate — ~650 words, complex sentences)",
        2 => "N2 (upper intermediate — ~1500 words)",
        1 => "N1 (advanced — ~2000+ words, native-level grammar)",
        _ => "N5 (beginner)",
    };

    let mut prompt = format!(
        "{}\n\nStudent level: {}\nTotal words encountered: {}",
        SYSTEM_BASE, level_desc, profile.total_words
    );

    if let Some(summary) = last_summary {
        prompt.push_str("\n\nPrevious lesson summary:\n");
        if !summary.topics_covered.is_empty() {
            prompt.push_str(&format!(
                "Topics covered: {}\n",
                summary.topics_covered.join(", ")
            ));
        }
        if !summary.continue_from.is_empty() {
            prompt.push_str(&format!("Continue from: {}\n", summary.continue_from));
        }
    }

    ChatMessage::system(prompt)
}
