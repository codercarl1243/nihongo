use std::sync::Arc;

pub trait LanguageConfig: Send + Sync {
    fn language_code(&self) -> &str;
    fn language_name(&self) -> &str;
    fn level_count(&self) -> u8;
    fn level_name(&self, level: u8) -> &str;
    fn level_description(&self, level: u8) -> &str;
    fn instruction_for_level(&self, level: u8) -> &str;
    fn silence_threshold_ms(&self, level: u8) -> u64;
    fn is_drill_prompt(&self, response: &str) -> bool;
    fn tts_voice(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Japanese implementation
// ---------------------------------------------------------------------------

pub struct Japanese;

impl LanguageConfig for Japanese {
    fn language_code(&self) -> &str { "ja" }
    fn language_name(&self) -> &str { "Japanese" }
    fn level_count(&self) -> u8 { 5 }

    fn level_name(&self, level: u8) -> &str {
        match level { 1 => "N1", 2 => "N2", 3 => "N3", 4 => "N4", _ => "N5" }
    }

    fn level_description(&self, level: u8) -> &str {
        match level {
            5 => "N5 (absolute beginner — hiragana, romaji, katakana, ~100 basic words)",
            4 => "N4 (elementary — ~300 words, basic grammar)",
            3 => "N3 (intermediate — ~650 words, complex sentences)",
            2 => "N2 (upper intermediate — ~1500 words)",
            1 => "N1 (advanced — ~2000+ words, native-level grammar)",
            _ => "N5 (beginner)",
        }
    }

    fn instruction_for_level(&self, level: u8) -> &str {
        match level {
            1 | 2 => INSTRUCTION_LANGUAGE_N2_N1,
            3     => INSTRUCTION_LANGUAGE_N3,
            _     => INSTRUCTION_LANGUAGE_N5_N4,
        }
    }

    fn silence_threshold_ms(&self, level: u8) -> u64 {
        // Beginners need more time to retrieve words; advanced learners can pace naturally.
        match level { 1 | 2 => 700, 3 => 900, _ => 1200 }
    }

    fn is_drill_prompt(&self, response: &str) -> bool {
        let r = response.to_lowercase();
        r.contains("try say")
            || r.contains("can you say")
            || r.contains("try using")
            || r.contains("how do you say")
            || r.contains("say that")
            || r.contains("repeat")
            || r.contains("言ってみて")
            || r.contains("言えますか")
    }

    fn tts_voice(&self) -> &str { "Ono_Anna" }
}

// ---------------------------------------------------------------------------
// Instruction-language fragments (Japanese-specific)
// ---------------------------------------------------------------------------

const INSTRUCTION_LANGUAGE_N5_N4: &str =
    "\n\nInstruction language: The student is a beginner and understands little or no \
     Japanese. Conduct the lesson in English. When you introduce a Japanese word or \
     phrase, say it in Japanese then immediately give the meaning in English in parentheses. \
     Never reply to a question with a Japanese-only sentence.\
     \n\nScript: Write all Japanese using romaji, until the student understands hiragana, and katakana. \
     Only use kanji when the student has already encountered that specific character in a lesson.";

const INSTRUCTION_LANGUAGE_N3: &str =
    "\n\nInstruction language: Mix English and Japanese. Use simple Japanese sentences \
     the student knows, but fall back to English for explanations. Always gloss new words.\
     \n\nScript: Use hiragana, katakana, and common everyday kanji (N3 level and below). \
     Write less familiar kanji in hiragana.";

const INSTRUCTION_LANGUAGE_N2_N1: &str =
    "\n\nInstruction language: Conduct the lesson in Japanese. Use English only when \
     explicitly asked or when a grammar point cannot be expressed otherwise.\
     \n\nScript: Use kanji, hiragana, and katakana naturally as a native speaker would.";

// ---------------------------------------------------------------------------
// Type alias for convenience
// ---------------------------------------------------------------------------

pub type LangConfig = Arc<dyn LanguageConfig>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_code_is_ja() {
        assert_eq!(Japanese.language_code(), "ja");
    }

    #[test]
    fn language_name_is_japanese() {
        assert_eq!(Japanese.language_name(), "Japanese");
    }

    #[test]
    fn level_name_roundtrip() {
        assert_eq!(Japanese.level_name(1), "N1");
        assert_eq!(Japanese.level_name(2), "N2");
        assert_eq!(Japanese.level_name(3), "N3");
        assert_eq!(Japanese.level_name(4), "N4");
        assert_eq!(Japanese.level_name(5), "N5");
        assert_eq!(Japanese.level_name(99), "N5"); // default
    }

    #[test]
    fn silence_threshold_beginner_is_1200() {
        assert_eq!(Japanese.silence_threshold_ms(5), 1200);
        assert_eq!(Japanese.silence_threshold_ms(4), 1200);
    }

    #[test]
    fn silence_threshold_intermediate_is_900() {
        assert_eq!(Japanese.silence_threshold_ms(3), 900);
    }

    #[test]
    fn silence_threshold_advanced_is_700() {
        assert_eq!(Japanese.silence_threshold_ms(1), 700);
        assert_eq!(Japanese.silence_threshold_ms(2), 700);
    }

    #[test]
    fn drill_prompt_detects_japanese_keyword() {
        assert!(Japanese.is_drill_prompt("言ってみて！"));
        assert!(Japanese.is_drill_prompt("言えますか？"));
    }

    #[test]
    fn drill_prompt_detects_english_keyword() {
        assert!(Japanese.is_drill_prompt("Can you say that in Japanese?"));
        assert!(Japanese.is_drill_prompt("Try using the て-form."));
        assert!(Japanese.is_drill_prompt("How do you say 'apple'?"));
        assert!(Japanese.is_drill_prompt("Please repeat after me."));
    }

    #[test]
    fn drill_prompt_rejects_non_drill() {
        assert!(!Japanese.is_drill_prompt("That is correct! Well done."));
        assert!(!Japanese.is_drill_prompt("Let's move on to the next topic."));
    }

    #[test]
    fn instruction_for_level_n5_uses_romaji_rule() {
        let instr = Japanese.instruction_for_level(5);
        assert!(instr.contains("romaji"), "N5 instruction should mention romaji");
        assert!(instr.contains("Only use kanji when"), "N5 instruction should conditionally allow kanji");
    }

    #[test]
    fn instruction_for_level_n3_allows_everyday_kanji() {
        let instr = Japanese.instruction_for_level(3);
        assert!(instr.contains("N3 level"), "N3 instruction should reference N3 kanji level");
        assert!(!instr.contains("native speaker"), "N3 instruction should not apply N2/N1 rule");
    }

    #[test]
    fn instruction_for_level_n1_uses_native_speaker_rule() {
        let instr = Japanese.instruction_for_level(1);
        assert!(instr.contains("native speaker"), "N1 instruction should use native-speaker rule");
    }
}
