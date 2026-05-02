use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerProfile {
    pub current_level: u8,   // 5 = N5 (beginner) … 1 = N1
    pub target_level: u8,
    pub total_words: i64,
}

impl Default for LearnerProfile {
    fn default() -> Self {
        Self { current_level: 5, target_level: 4, total_words: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabEntry {
    pub word: String,
    pub reading: Option<String>,
    pub meaning: Option<String>,
    pub jlpt_level: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonSummary {
    pub id: i64,
    pub topics_covered: Vec<String>,
    pub words_introduced: Vec<i64>,
    pub words_reviewed: Vec<i64>,
    pub continue_from: String,
}
