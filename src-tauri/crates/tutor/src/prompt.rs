use db::{LearnerProfile, LessonSummary, SessionContext};
use llm::ChatMessage;

use crate::language::LanguageConfig;

const SYSTEM_BASE: &str = "\
You are a Japanese language tutor in a voice conversation. Keep every reply SHORT \
— one or two sentences maximum, no exceptions.

Rules:
- Respond naturally using vocabulary at or below the student's JLPT level and words \
  they have already encountered — but do not explain them unless asked.
- Greetings get a one-sentence greeting back, nothing more.
- When correcting, stay grounded in what the student was trying to say given the \
  conversation context. If their words seem off-topic (e.g. they say 天気 when asked \
  how they are), assume they confused a similar word (元気) rather than changing subject \
  — gently redirect to what they likely intended.
- When the conversation reaches a natural pause or teaching moment, end your response \
  with a short prompt inviting the student to try something in Japanese. Use your \
  judgement — casual exchanges like greetings do not need a prompt.";

pub fn build_system_prompt_pub(ctx: &SessionContext, lang: &dyn LanguageConfig) -> ChatMessage {
    build_system_prompt(ctx, lang)
}

pub fn build_system_prompt(ctx: &SessionContext, lang: &dyn LanguageConfig) -> ChatMessage {
    let level = ctx.profile.current_level;
    let level_desc    = lang.level_description(level);
    let instruction_lang = lang.instruction_for_level(level);

    let mut prompt = format!(
        "{}{}\n\nStudent level: {}\nTotal words encountered: {}",
        SYSTEM_BASE, instruction_lang, level_desc, ctx.profile.total_words
    );

    if let Some(ref active) = ctx.current_topic {
        prompt.push_str(&format!(
            "\n\nCurrent topic: {} — {}",
            active.topic.name, active.topic.description
        ));

        // First 5 words to introduce in this topic
        let to_introduce: Vec<String> = active.pending.iter().take(5)
            .filter_map(|v| v.meaning.as_ref().map(|m| format!("{} ({})", v.word, m)))
            .collect();
        if !to_introduce.is_empty() {
            prompt.push_str(&format!("\nWords to introduce: {}", to_introduce.join(", ")));
        }

        // Words the student has already seen in this topic
        if !active.introduced.is_empty() {
            let seen: Vec<&str> = active.introduced.iter().take(10)
                .map(|v| v.word.as_str())
                .collect();
            prompt.push_str(&format!("\nWords already seen in topic: {}", seen.join(", ")));
        }
    }

    // SRS words due for review today
    if !ctx.srs_due.is_empty() {
        let due: Vec<&str> = ctx.srs_due.iter().map(|v| v.word.as_str()).collect();
        prompt.push_str(&format!("\nWords due for review today: {}", due.join(", ")));
    }

    // Previous lesson notes
    if let Some(ref notes) = ctx.last_notes {
        prompt.push_str("\n\nPrevious lesson notes:\n");
        prompt.push_str(notes);
    }

    ChatMessage::system(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Japanese;
    use db::{LearnerProfile, SessionContext};

    fn ctx(level: u8) -> SessionContext {
        SessionContext {
            profile: LearnerProfile { current_level: level, target_level: 1, total_words: 0 },
            current_topic: None,
            srs_due: vec![],
            last_notes: None,
        }
    }

    fn prompt_text(level: u8) -> String {
        build_system_prompt(&ctx(level), &Japanese).content
    }

    #[test]
    fn n5_prompt_uses_romaji_rule() {
        let text = prompt_text(5);
        assert!(text.contains("romaji"), "N5 prompt should mention romaji");
        assert!(text.contains("Only use kanji when"), "N5 prompt should conditionally allow kanji");
    }

    #[test]
    fn n4_prompt_uses_romaji_rule() {
        let text = prompt_text(4);
        assert!(text.contains("romaji"), "N4 prompt should mention romaji");
        assert!(text.contains("Only use kanji when"), "N4 prompt should conditionally allow kanji");
    }

    #[test]
    fn n3_prompt_allows_everyday_kanji() {
        let text = prompt_text(3);
        assert!(text.contains("N3 level"), "N3 prompt missing N3 kanji instruction");
        assert!(!text.contains("Only use kanji when"), "N3 prompt should not ban kanji");
        assert!(!text.contains("as a native speaker would"), "N3 prompt should not apply N2/N1 instruction");
    }

    #[test]
    fn n2_prompt_uses_full_kanji() {
        let text = prompt_text(2);
        assert!(text.contains("as a native speaker would"), "N2 prompt missing native-speaker script instruction");
        assert!(!text.contains("Only use kanji when"), "N2 prompt should not ban kanji");
    }

    #[test]
    fn n1_prompt_uses_full_kanji() {
        let text = prompt_text(1);
        assert!(text.contains("as a native speaker would"), "N1 prompt missing native-speaker script instruction");
    }

    #[test]
    fn prompt_includes_student_level_description() {
        assert!(prompt_text(5).contains("N5"));
        assert!(prompt_text(4).contains("N4"));
        assert!(prompt_text(3).contains("N3"));
        assert!(prompt_text(2).contains("N2"));
        assert!(prompt_text(1).contains("N1"));
    }

    #[test]
    fn n5_n4_instruction_language_is_english() {
        for level in [4u8, 5] {
            let text = prompt_text(level);
            assert!(text.contains("Conduct the lesson in English"), "level {level} should instruct English");
        }
    }

    #[test]
    fn n2_n1_instruction_language_is_japanese() {
        for level in [1u8, 2] {
            let text = prompt_text(level);
            assert!(text.contains("Conduct the lesson in Japanese"), "level {level} should instruct Japanese");
        }
    }

    #[test]
    fn prompt_includes_srs_due_words() {
        use db::StudentVocabulary;
        let mut ctx = ctx(2);
        ctx.srs_due = vec![
            StudentVocabulary { id: 1, vocabulary_id: 1, word: "猫".into(), reading: None, meaning: None, fluency_level: 0, times_correct: 0, times_incorrect: 0 },
            StudentVocabulary { id: 2, vocabulary_id: 2, word: "犬".into(), reading: None, meaning: None, fluency_level: 0, times_correct: 0, times_incorrect: 0 },
        ];
        let text = build_system_prompt(&ctx, &Japanese).content;
        assert!(text.contains("猫"), "prompt should list SRS due word 猫");
        assert!(text.contains("犬"), "prompt should list SRS due word 犬");
    }

    #[test]
    fn prompt_includes_previous_notes() {
        let mut ctx = ctx(3);
        ctx.last_notes = Some("Student struggles with て-form.".into());
        let text = build_system_prompt(&ctx, &Japanese).content;
        assert!(text.contains("Student struggles with て-form."));
    }
}

// ---------------------------------------------------------------------------
// Legacy helper — kept for callers that still have separate profile + summary.
// Remove once all call sites move to SessionContext.
// ---------------------------------------------------------------------------

pub fn build_system_prompt_from_parts(
    profile: &LearnerProfile,
    last_summary: Option<&LessonSummary>,
    lang: &dyn LanguageConfig,
) -> ChatMessage {
    let ctx = SessionContext {
        profile: profile.clone(),
        current_topic: None,
        srs_due: vec![],
        last_notes: last_summary
            .filter(|s| !s.notes.is_empty())
            .map(|s| s.notes.clone()),
    };
    build_system_prompt(&ctx, lang)
}
