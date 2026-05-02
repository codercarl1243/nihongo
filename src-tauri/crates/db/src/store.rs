use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

use crate::types::{LearnerProfile, LessonSummary, VocabEntry};

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open db at {}", path.display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS sessions (
                id                INTEGER PRIMARY KEY,
                started_at        DATETIME DEFAULT CURRENT_TIMESTAMP,
                ended_at          DATETIME,
                lesson_summary_id INTEGER REFERENCES lesson_summaries(id)
            );

            CREATE TABLE IF NOT EXISTS conversation_turns (
                id              INTEGER PRIMARY KEY,
                session_id      INTEGER REFERENCES sessions(id),
                created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
                student_input   TEXT,
                tutor_response  TEXT,
                milestone       BOOLEAN DEFAULT FALSE
            );

            CREATE TABLE IF NOT EXISTS lesson_summaries (
                id               INTEGER PRIMARY KEY,
                created_at       DATETIME DEFAULT CURRENT_TIMESTAMP,
                topics_covered   TEXT,
                words_introduced TEXT,
                words_reviewed   TEXT,
                continue_from    TEXT
            );

            CREATE TABLE IF NOT EXISTS vocabulary (
                id         INTEGER PRIMARY KEY,
                word       TEXT NOT NULL UNIQUE,
                reading    TEXT,
                meaning    TEXT,
                jlpt_level INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS encounters (
                id            INTEGER PRIMARY KEY,
                vocabulary_id INTEGER REFERENCES vocabulary(id),
                session_id    INTEGER REFERENCES sessions(id),
                encountered_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                context       TEXT,
                understood    BOOLEAN
            );

            CREATE TABLE IF NOT EXISTS srs_schedule (
                vocabulary_id INTEGER PRIMARY KEY REFERENCES vocabulary(id),
                interval_days REAL    DEFAULT 1,
                ease_factor   REAL    DEFAULT 2.5,
                due_at        DATETIME,
                streak        INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS learner_profile (
                id            INTEGER PRIMARY KEY CHECK(id = 1),
                current_level INTEGER DEFAULT 5,
                target_level  INTEGER DEFAULT 4,
                total_words   INTEGER DEFAULT 0,
                updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            INSERT OR IGNORE INTO learner_profile(id) VALUES (1);
        ")?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------------

    pub fn start_session(&self) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions DEFAULT VALUES",
            [],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn end_session(&self, session_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET ended_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Turns
    // -----------------------------------------------------------------------

    pub fn record_turn(
        &self,
        session_id: i64,
        student_input: &str,
        tutor_response: &str,
        milestone: bool,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO conversation_turns (session_id, student_input, tutor_response, milestone)
             VALUES (?1, ?2, ?3, ?4)",
            params![session_id, student_input, tutor_response, milestone],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // -----------------------------------------------------------------------
    // Vocabulary
    // -----------------------------------------------------------------------

    pub fn upsert_vocabulary(&self, entry: &VocabEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO vocabulary (word, reading, meaning, jlpt_level)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(word) DO NOTHING",
            params![entry.word, entry.reading, entry.meaning, entry.jlpt_level],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM vocabulary WHERE word = ?1",
            params![entry.word],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    pub fn record_encounter(
        &self,
        vocab_id: i64,
        session_id: i64,
        context: &str,
        understood: bool,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO encounters (vocabulary_id, session_id, context, understood)
             VALUES (?1, ?2, ?3, ?4)",
            params![vocab_id, session_id, context, understood],
        )?;
        self.update_srs(vocab_id, understood)?;
        Ok(())
    }

    fn update_srs(&self, vocab_id: i64, understood: bool) -> Result<()> {
        // Ensure a row exists
        self.conn.execute(
            "INSERT OR IGNORE INTO srs_schedule (vocabulary_id, due_at)
             VALUES (?1, datetime('now', '+1 day'))",
            params![vocab_id],
        )?;

        if understood {
            self.conn.execute(
                "UPDATE srs_schedule
                 SET streak        = streak + 1,
                     ease_factor   = MIN(ease_factor + 0.1, 4.0),
                     interval_days = interval_days * ease_factor,
                     due_at        = datetime('now', '+' || CAST(ROUND(interval_days * ease_factor) AS TEXT) || ' days')
                 WHERE vocabulary_id = ?1",
                params![vocab_id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE srs_schedule
                 SET streak        = 0,
                     ease_factor   = MAX(ease_factor - 0.2, 1.3),
                     interval_days = 1,
                     due_at        = datetime('now', '+1 day')
                 WHERE vocabulary_id = ?1",
                params![vocab_id],
            )?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Lesson summaries
    // -----------------------------------------------------------------------

    pub fn save_lesson_summary(&self, summary: &LessonSummary) -> Result<i64> {
        let topics = serde_json::to_string(&summary.topics_covered)?;
        let introduced = serde_json::to_string(&summary.words_introduced)?;
        let reviewed = serde_json::to_string(&summary.words_reviewed)?;

        self.conn.execute(
            "INSERT INTO lesson_summaries (topics_covered, words_introduced, words_reviewed, continue_from)
             VALUES (?1, ?2, ?3, ?4)",
            params![topics, introduced, reviewed, summary.continue_from],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn latest_lesson_summary(&self) -> Result<Option<LessonSummary>> {
        let result = self.conn.query_row(
            "SELECT id, topics_covered, words_introduced, words_reviewed, continue_from
             FROM lesson_summaries ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        );

        match result {
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
            Ok((id, topics, introduced, reviewed, cont)) => Ok(Some(LessonSummary {
                id,
                topics_covered: serde_json::from_str(&topics).unwrap_or_default(),
                words_introduced: serde_json::from_str(&introduced).unwrap_or_default(),
                words_reviewed: serde_json::from_str(&reviewed).unwrap_or_default(),
                continue_from: cont,
            })),
        }
    }

    // -----------------------------------------------------------------------
    // Learner profile
    // -----------------------------------------------------------------------

    pub fn learner_profile(&self) -> Result<LearnerProfile> {
        self.conn.query_row(
            "SELECT current_level, target_level, total_words FROM learner_profile WHERE id = 1",
            [],
            |r| Ok(LearnerProfile {
                current_level: r.get::<_, u8>(0)?,
                target_level: r.get::<_, u8>(1)?,
                total_words: r.get::<_, i64>(2)?,
            }),
        ).map_err(Into::into)
    }
}
