use db::{LearnerProfile, LessonSummary, SessionContext};
use llm::ChatMessage;

const SYSTEM_BASE: &str = "\
You are a Japanese language tutor in a voice conversation. Keep every reply SHORT \
— one or two sentences maximum, no exceptions.

Rules:
- Respond naturally using vocabulary at or below the student's JLPT level and words \
  they have already encountered — but do not explain them unless asked.
- Never volunteer definitions, grammar notes, or new vocabulary the student did not \
  ask about.
- Greetings get a one-sentence greeting back, nothing more.
- Only correct or explicitly teach when the student makes an attempt or asks a question.
- When correcting, stay grounded in what the student was trying to say given the \
  conversation context. If their words seem off-topic (e.g. they say 天気 when asked \
  how they are), assume they confused a similar word (元気) rather than changing subject \
  — gently redirect to what they likely intended.
- When the conversation reaches a natural pause or teaching moment, end your response \
  with a short prompt inviting the student to try something in Japanese. Use your \
  judgement — casual exchanges like greetings do not need a prompt.";

const INSTRUCTION_LANGUAGE_N5_N4: &str =
    "\n\nInstruction language: The student is a beginner and understands little or no \
     Japanese. Conduct the lesson in English. When you introduce a Japanese word or \
     phrase, say it in Japanese then immediately give the English meaning in parentheses. \
     Never reply to an English question with a Japanese-only sentence.\
     \n\nScript: Write all Japanese using hiragana and katakana only. Do not use any kanji.";

const INSTRUCTION_LANGUAGE_N3: &str =
    "\n\nInstruction language: Mix English and Japanese. Use simple Japanese sentences \
     the student knows, but fall back to English for explanations. Always gloss new words.\
     \n\nScript: Use hiragana, katakana, and common everyday kanji (N3 level and below). \
     Write less familiar kanji in hiragana.";

const INSTRUCTION_LANGUAGE_N2_N1: &str =
    "\n\nInstruction language: Conduct the lesson in Japanese. Use English only when \
     explicitly asked or when a grammar point cannot be expressed otherwise.\
     \n\nScript: Use kanji, hiragana, and katakana naturally as a native speaker would.";

pub fn build_system_prompt_pub(ctx: &SessionContext) -> ChatMessage {
    build_system_prompt(ctx)
}

pub fn build_system_prompt(ctx: &SessionContext) -> ChatMessage {
    let level_desc = match ctx.profile.current_level {
        5 => "N5 (absolute beginner — hiragana, katakana, ~100 basic words)",
        4 => "N4 (elementary — ~300 words, basic grammar)",
        3 => "N3 (intermediate — ~650 words, complex sentences)",
        2 => "N2 (upper intermediate — ~1500 words)",
        1 => "N1 (advanced — ~2000+ words, native-level grammar)",
        _ => "N5 (beginner)",
    };

    let instruction_lang = match ctx.profile.current_level {
        1 | 2 => INSTRUCTION_LANGUAGE_N2_N1,
        3     => INSTRUCTION_LANGUAGE_N3,
        _     => INSTRUCTION_LANGUAGE_N5_N4,
    };

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

// ---------------------------------------------------------------------------
// Legacy helper — kept for callers that still have separate profile + summary.
// Remove once all call sites move to SessionContext.
// ---------------------------------------------------------------------------

pub fn build_system_prompt_from_parts(
    profile: &LearnerProfile,
    last_summary: Option<&LessonSummary>,
) -> ChatMessage {
    let ctx = SessionContext {
        profile: profile.clone(),
        current_topic: None,
        srs_due: vec![],
        last_notes: last_summary
            .filter(|s| !s.notes.is_empty())
            .map(|s| s.notes.clone()),
    };
    build_system_prompt(&ctx)
}
