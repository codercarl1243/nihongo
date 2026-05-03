use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Learner
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerProfile {
    pub current_level: u8, // 5 = N5 (beginner) … 1 = N1
    pub target_level:  u8,
    pub total_words:   i64,
}

impl Default for LearnerProfile {
    fn default() -> Self {
        Self { current_level: 5, target_level: 4, total_words: 0 }
    }
}

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabEntry {
    pub id:         i64,  // 0 for new inserts; set by DB on read-back
    pub word:       String,
    pub reading:    Option<String>,
    pub meaning:    Option<String>,
    pub jlpt_level: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentVocabulary {
    pub id:              i64,
    pub vocabulary_id:   i64,
    pub word:            String,
    pub reading:         Option<String>,
    pub meaning:         Option<String>,
    pub fluency_level:   u8,  // 0–10; topic complete when all words hit 10
    pub times_correct:   i64,
    pub times_incorrect: i64,
}

// ---------------------------------------------------------------------------
// Kanji
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanjiEntry {
    pub id:           i64,
    pub character:    String,
    pub onyomi:       Option<String>,
    pub kunyomi:      Option<String>,
    pub meaning:      String,
    pub jlpt_level:   Option<u8>,
    pub joyo_grade:   Option<u8>,
    pub stroke_count: Option<u8>,
    pub radicals:     Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentKanji {
    pub id:              i64,
    pub kanji_id:        i64,
    pub character:       String,
    pub meaning:         String,
    pub fluency_level:   u8,
    pub times_correct:   i64,
    pub times_incorrect: i64,
}

// ---------------------------------------------------------------------------
// Topics & curriculum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TopicStatus {
    NotStarted,
    InProgress,
    Completed,
}

impl TopicStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TopicStatus::NotStarted => "not_started",
            TopicStatus::InProgress => "in_progress",
            TopicStatus::Completed  => "completed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => TopicStatus::InProgress,
            "completed"   => TopicStatus::Completed,
            _             => TopicStatus::NotStarted,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id:             i64,
    pub jlpt_level:     u8,
    pub sequence_order: i64,
    pub name:           String,
    pub description:    String,
    pub topic_type:     String, // 'vocabulary' | 'kanji'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentTopicProgress {
    pub topic_id:     i64,
    pub topic_name:   String,
    pub status:       TopicStatus,
    pub started_at:   Option<String>,
    pub completed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Lesson summaries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VocabOutcome {
    Introduced,
    Correct,
    Incorrect,
}

impl VocabOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            VocabOutcome::Introduced => "introduced",
            VocabOutcome::Correct    => "correct",
            VocabOutcome::Incorrect  => "incorrect",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonSummaryVocab {
    pub vocabulary_id: i64,
    pub word:          String,
    pub outcome:       String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonSummaryTopic {
    pub topic_id:   i64,
    pub topic_name: String,
    pub status:     String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonSummary {
    pub id:         i64,
    pub session_id: Option<i64>,
    pub notes:      String,
    pub topics:     Vec<LessonSummaryTopic>,
    pub vocabulary: Vec<LessonSummaryVocab>,
}

// ---------------------------------------------------------------------------
// Session context — what the system prompt builder needs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub profile:       LearnerProfile,
    pub current_topic: Option<ActiveTopic>,
    pub srs_due:       Vec<StudentVocabulary>,
    pub last_notes:    Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTopic {
    pub topic:      Topic,
    pub introduced: Vec<StudentVocabulary>, // words seen in this topic
    pub pending:    Vec<VocabEntry>,        // words not yet introduced
}
