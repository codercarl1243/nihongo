use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

use crate::{
    seed::N5_SEED_SQL,
    types::{
        ActiveTopic, LearnerProfile, LessonSummary, LessonSummaryTopic,
        LessonSummaryVocab, SessionContext, StudentVocabulary, Topic,
        TopicStatus, VocabEntry,
    },
};

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
        let version: i32 = self.conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            self.migrate_to_v1()?;
        }
        if version < 2 {
            self.migrate_to_v2()?;
        }
        Ok(())
    }

    fn migrate_to_v2(&self) -> Result<()> {
        self.conn.execute_batch("
            ALTER TABLE topics             ADD COLUMN language_code TEXT NOT NULL DEFAULT 'ja';
            ALTER TABLE vocabulary         ADD COLUMN language_code TEXT NOT NULL DEFAULT 'ja';
            ALTER TABLE student_vocabulary ADD COLUMN language_code TEXT NOT NULL DEFAULT 'ja';
            PRAGMA user_version = 2;
        ")?;
        Ok(())
    }

    fn migrate_to_v1(&self) -> Result<()> {
        // Drop old schema (dev DB — no prod data to preserve).
        self.conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        self.conn.execute_batch("
            DROP TABLE IF EXISTS encounters;
            DROP TABLE IF EXISTS srs_schedule;
            DROP TABLE IF EXISTS lesson_summaries;
            DROP TABLE IF EXISTS vocabulary;
            DROP TABLE IF EXISTS sessions;
            DROP TABLE IF EXISTS conversation_turns;
            DROP TABLE IF EXISTS learner_profile;
        ")?;

        self.conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            -- ── Core profile ────────────────────────────────────────────────
            CREATE TABLE learner_profile (
                id            INTEGER PRIMARY KEY CHECK(id = 1),
                current_level INTEGER NOT NULL DEFAULT 5,
                target_level  INTEGER NOT NULL DEFAULT 4,
                total_words   INTEGER NOT NULL DEFAULT 0,
                updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO learner_profile(id) VALUES (1);

            -- ── Session & turns ─────────────────────────────────────────────
            CREATE TABLE sessions (
                id         INTEGER PRIMARY KEY,
                started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                ended_at   DATETIME
            );

            CREATE TABLE conversation_turns (
                id             INTEGER PRIMARY KEY,
                session_id     INTEGER REFERENCES sessions(id),
                created_at     DATETIME DEFAULT CURRENT_TIMESTAMP,
                student_input  TEXT,
                tutor_response TEXT,
                milestone      BOOLEAN DEFAULT FALSE
            );

            -- ── Vocabulary & kanji ──────────────────────────────────────────
            CREATE TABLE vocabulary (
                id         INTEGER PRIMARY KEY,
                word       TEXT NOT NULL UNIQUE,
                reading    TEXT,
                meaning    TEXT,
                jlpt_level INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE kanji (
                id           INTEGER PRIMARY KEY,
                character    TEXT NOT NULL UNIQUE,
                onyomi       TEXT,
                kunyomi      TEXT,
                meaning      TEXT NOT NULL,
                jlpt_level   INTEGER,
                joyo_grade   INTEGER,
                stroke_count INTEGER,
                radicals     TEXT
            );

            -- ── Per-student progress ────────────────────────────────────────
            CREATE TABLE student_vocabulary (
                id              INTEGER PRIMARY KEY,
                vocabulary_id   INTEGER NOT NULL UNIQUE REFERENCES vocabulary(id),
                fluency_level   INTEGER NOT NULL DEFAULT 0,
                times_correct   INTEGER NOT NULL DEFAULT 0,
                times_incorrect INTEGER NOT NULL DEFAULT 0,
                last_seen_at    DATETIME
            );

            CREATE TABLE student_kanji (
                id              INTEGER PRIMARY KEY,
                kanji_id        INTEGER NOT NULL UNIQUE REFERENCES kanji(id),
                fluency_level   INTEGER NOT NULL DEFAULT 0,
                times_correct   INTEGER NOT NULL DEFAULT 0,
                times_incorrect INTEGER NOT NULL DEFAULT 0,
                last_seen_at    DATETIME
            );

            -- ── SRS (covers both vocabulary and kanji) ──────────────────────
            CREATE TABLE srs_schedule (
                id            INTEGER PRIMARY KEY,
                item_type     TEXT NOT NULL,
                item_id       INTEGER NOT NULL,
                interval_days REAL    NOT NULL DEFAULT 1,
                ease_factor   REAL    NOT NULL DEFAULT 2.5,
                due_at        DATETIME,
                streak        INTEGER NOT NULL DEFAULT 0,
                UNIQUE(item_type, item_id)
            );

            -- ── Curriculum ──────────────────────────────────────────────────
            CREATE TABLE topics (
                id             INTEGER PRIMARY KEY,
                jlpt_level     INTEGER NOT NULL,
                sequence_order INTEGER NOT NULL,
                name           TEXT NOT NULL,
                description    TEXT NOT NULL,
                topic_type     TEXT NOT NULL
            );

            CREATE TABLE topic_dependencies (
                topic_id            INTEGER NOT NULL REFERENCES topics(id),
                depends_on_topic_id INTEGER NOT NULL REFERENCES topics(id),
                PRIMARY KEY (topic_id, depends_on_topic_id)
            );

            CREATE TABLE topic_vocabulary (
                topic_id       INTEGER NOT NULL REFERENCES topics(id),
                vocabulary_id  INTEGER NOT NULL REFERENCES vocabulary(id),
                sequence_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (topic_id, vocabulary_id)
            );

            CREATE TABLE topic_kanji (
                topic_id       INTEGER NOT NULL REFERENCES topics(id),
                kanji_id       INTEGER NOT NULL REFERENCES kanji(id),
                sequence_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (topic_id, kanji_id)
            );

            CREATE TABLE vocabulary_kanji (
                vocabulary_id INTEGER NOT NULL REFERENCES vocabulary(id),
                kanji_id      INTEGER NOT NULL REFERENCES kanji(id),
                PRIMARY KEY (vocabulary_id, kanji_id)
            );

            CREATE TABLE student_topic_progress (
                topic_id     INTEGER PRIMARY KEY REFERENCES topics(id),
                status       TEXT NOT NULL DEFAULT 'not_started',
                started_at   DATETIME,
                completed_at DATETIME
            );

            -- ── Lesson summaries (structured) ───────────────────────────────
            CREATE TABLE lesson_summaries (
                id         INTEGER PRIMARY KEY,
                session_id INTEGER REFERENCES sessions(id),
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                notes      TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE lesson_summary_topics (
                id                INTEGER PRIMARY KEY,
                lesson_summary_id INTEGER NOT NULL REFERENCES lesson_summaries(id),
                topic_id          INTEGER REFERENCES topics(id),
                topic_name        TEXT NOT NULL,
                status            TEXT NOT NULL
            );

            CREATE TABLE lesson_summary_vocabulary (
                id                INTEGER PRIMARY KEY,
                lesson_summary_id INTEGER NOT NULL REFERENCES lesson_summaries(id),
                vocabulary_id     INTEGER NOT NULL REFERENCES vocabulary(id),
                word              TEXT NOT NULL,
                outcome           TEXT NOT NULL
            );

            -- ── Lesson plans ────────────────────────────────────────────────
            CREATE TABLE lesson_plans (
                id         INTEGER PRIMARY KEY,
                jlpt_level INTEGER NOT NULL,
                title      TEXT NOT NULL
            );

            CREATE TABLE lesson_plan_topics (
                lesson_plan_id INTEGER NOT NULL REFERENCES lesson_plans(id),
                topic_id       INTEGER NOT NULL REFERENCES topics(id),
                sequence_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (lesson_plan_id, topic_id)
            );
        ")?;

        // OR IGNORE does not suppress FK violations in SQLite, so disable FK
        // checks while inserting pre-validated seed data.
        self.conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        self.conn.execute_batch(N5_SEED_SQL)?;
        self.conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        self.conn.execute_batch("PRAGMA user_version = 1;")?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------------

    pub fn start_session(&self) -> Result<i64> {
        self.conn.execute("INSERT INTO sessions DEFAULT VALUES", [])?;
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
    // Learner profile
    // -----------------------------------------------------------------------

    pub fn learner_profile(&self) -> Result<LearnerProfile> {
        self.conn.query_row(
            "SELECT current_level, target_level, total_words
             FROM learner_profile WHERE id = 1",
            [],
            |r| Ok(LearnerProfile {
                current_level: r.get::<_, u8>(0)?,
                target_level:  r.get::<_, u8>(1)?,
                total_words:   r.get::<_, i64>(2)?,
            }),
        ).map_err(Into::into)
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

    /// Mark a word as introduced to the student; safe to call multiple times.
    pub fn introduce_word(&self, vocabulary_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO student_vocabulary (vocabulary_id, last_seen_at)
             VALUES (?1, CURRENT_TIMESTAMP)",
            params![vocabulary_id],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO srs_schedule (item_type, item_id, due_at)
             VALUES ('vocabulary', ?1, datetime('now', '+1 day'))",
            params![vocabulary_id],
        )?;
        Ok(())
    }

    /// Record an attempt on a vocabulary word and update fluency + SRS.
    pub fn update_word_fluency(&self, vocabulary_id: i64, correct: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO student_vocabulary (vocabulary_id, last_seen_at)
             VALUES (?1, CURRENT_TIMESTAMP)",
            params![vocabulary_id],
        )?;
        if correct {
            self.conn.execute(
                "UPDATE student_vocabulary
                 SET fluency_level   = MIN(fluency_level + 1, 10),
                     times_correct   = times_correct + 1,
                     last_seen_at    = CURRENT_TIMESTAMP
                 WHERE vocabulary_id = ?1",
                params![vocabulary_id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE student_vocabulary
                 SET fluency_level   = MAX(fluency_level - 1, 0),
                     times_incorrect = times_incorrect + 1,
                     last_seen_at    = CURRENT_TIMESTAMP
                 WHERE vocabulary_id = ?1",
                params![vocabulary_id],
            )?;
        }
        self.update_srs("vocabulary", vocabulary_id, correct)
    }

    /// Record an attempt on a kanji and update fluency + SRS.
    pub fn update_kanji_fluency(&self, kanji_id: i64, correct: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO student_kanji (kanji_id, last_seen_at)
             VALUES (?1, CURRENT_TIMESTAMP)",
            params![kanji_id],
        )?;
        if correct {
            self.conn.execute(
                "UPDATE student_kanji
                 SET fluency_level   = MIN(fluency_level + 1, 10),
                     times_correct   = times_correct + 1,
                     last_seen_at    = CURRENT_TIMESTAMP
                 WHERE kanji_id = ?1",
                params![kanji_id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE student_kanji
                 SET fluency_level   = MAX(fluency_level - 1, 0),
                     times_incorrect = times_incorrect + 1,
                     last_seen_at    = CURRENT_TIMESTAMP
                 WHERE kanji_id = ?1",
                params![kanji_id],
            )?;
        }
        self.update_srs("kanji", kanji_id, correct)
    }

    fn update_srs(&self, item_type: &str, item_id: i64, correct: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO srs_schedule (item_type, item_id, due_at)
             VALUES (?1, ?2, datetime('now', '+1 day'))",
            params![item_type, item_id],
        )?;
        if correct {
            self.conn.execute(
                "UPDATE srs_schedule
                 SET streak        = streak + 1,
                     ease_factor   = MIN(ease_factor + 0.1, 4.0),
                     interval_days = interval_days * ease_factor,
                     due_at = datetime('now',
                                '+' || CAST(ROUND(interval_days * ease_factor) AS TEXT) || ' days')
                 WHERE item_type = ?1 AND item_id = ?2",
                params![item_type, item_id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE srs_schedule
                 SET streak        = 0,
                     ease_factor   = MAX(ease_factor - 0.2, 1.3),
                     interval_days = 1,
                     due_at        = datetime('now', '+1 day')
                 WHERE item_type = ?1 AND item_id = ?2",
                params![item_type, item_id],
            )?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Session context
    // -----------------------------------------------------------------------

    pub fn session_context(&self) -> Result<SessionContext> {
        let profile     = self.learner_profile()?;
        let current_topic = self.active_topic()?;
        let srs_due     = self.vocabulary_due_for_review(5)?;
        let last_notes  = self.latest_lesson_summary()?
            .map(|s| s.notes)
            .filter(|n| !n.is_empty());
        Ok(SessionContext { profile, current_topic, srs_due, last_notes })
    }

    fn active_topic(&self) -> Result<Option<ActiveTopic>> {
        let topic = match self.topic_by_status("in_progress")? {
            Some(t) => t,
            None    => match self.next_available_topic()? {
                Some(t) => t,
                None    => return Ok(None),
            },
        };
        let introduced = self.topic_introduced_vocab(topic.id)?;
        let pending    = self.topic_pending_vocab(topic.id)?;
        Ok(Some(ActiveTopic { topic, introduced, pending }))
    }

    fn topic_by_status(&self, status: &str) -> Result<Option<Topic>> {
        let r = self.conn.query_row(
            "SELECT t.id, t.jlpt_level, t.sequence_order, t.name, t.description, t.topic_type
             FROM topics t
             JOIN student_topic_progress p ON p.topic_id = t.id
             WHERE p.status = ?1
             ORDER BY p.started_at DESC
             LIMIT 1",
            params![status],
            |r| self.row_to_topic(r),
        );
        match r {
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
            Ok(t)  => Ok(Some(t)),
        }
    }

    /// First topic whose dependencies are all completed (or none) and that the
    /// student has not yet started. Prefers lower JLPT levels (N5 first).
    fn next_available_topic(&self) -> Result<Option<Topic>> {
        let r = self.conn.query_row(
            "SELECT t.id, t.jlpt_level, t.sequence_order, t.name, t.description, t.topic_type
             FROM topics t
             WHERE t.id NOT IN (SELECT topic_id FROM student_topic_progress)
               AND NOT EXISTS (
                   SELECT 1 FROM topic_dependencies d
                   WHERE d.topic_id = t.id
                     AND d.depends_on_topic_id NOT IN (
                         SELECT topic_id FROM student_topic_progress
                         WHERE status = 'completed'
                     )
               )
             ORDER BY t.jlpt_level DESC, t.sequence_order ASC
             LIMIT 1",
            [],
            |r| self.row_to_topic(r),
        );
        match r {
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
            Ok(t)  => Ok(Some(t)),
        }
    }

    fn row_to_topic(&self, r: &rusqlite::Row<'_>) -> rusqlite::Result<Topic> {
        Ok(Topic {
            id:             r.get(0)?,
            jlpt_level:     r.get(1)?,
            sequence_order: r.get(2)?,
            name:           r.get(3)?,
            description:    r.get(4)?,
            topic_type:     r.get(5)?,
        })
    }

    fn topic_introduced_vocab(&self, topic_id: i64) -> Result<Vec<StudentVocabulary>> {
        let mut stmt = self.conn.prepare(
            "SELECT sv.id, sv.vocabulary_id, v.word, v.reading, v.meaning,
                    sv.fluency_level, sv.times_correct, sv.times_incorrect
             FROM student_vocabulary sv
             JOIN vocabulary v ON v.id = sv.vocabulary_id
             JOIN topic_vocabulary tv ON tv.vocabulary_id = sv.vocabulary_id
             WHERE tv.topic_id = ?1
             ORDER BY tv.sequence_order",
        )?;
        let rows = stmt.query_map(params![topic_id], |r| Ok(StudentVocabulary {
            id:              r.get(0)?,
            vocabulary_id:   r.get(1)?,
            word:            r.get(2)?,
            reading:         r.get(3)?,
            meaning:         r.get(4)?,
            fluency_level:   r.get(5)?,
            times_correct:   r.get(6)?,
            times_incorrect: r.get(7)?,
        }))?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn topic_pending_vocab(&self, topic_id: i64) -> Result<Vec<VocabEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.id, v.word, v.reading, v.meaning, v.jlpt_level
             FROM vocabulary v
             JOIN topic_vocabulary tv ON tv.vocabulary_id = v.id
             WHERE tv.topic_id = ?1
               AND v.id NOT IN (SELECT vocabulary_id FROM student_vocabulary)
             ORDER BY tv.sequence_order",
        )?;
        let rows = stmt.query_map(params![topic_id], |r| Ok(VocabEntry {
            id:         r.get(0)?,
            word:       r.get(1)?,
            reading:    r.get(2)?,
            meaning:    r.get(3)?,
            jlpt_level: r.get(4)?,
        }))?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn vocabulary_due_for_review(&self, limit: i64) -> Result<Vec<StudentVocabulary>> {
        let mut stmt = self.conn.prepare(
            "SELECT sv.id, sv.vocabulary_id, v.word, v.reading, v.meaning,
                    sv.fluency_level, sv.times_correct, sv.times_incorrect
             FROM student_vocabulary sv
             JOIN vocabulary v ON v.id = sv.vocabulary_id
             JOIN srs_schedule s ON s.item_type = 'vocabulary' AND s.item_id = sv.vocabulary_id
             WHERE s.due_at <= datetime('now')
               AND sv.fluency_level < 10
             ORDER BY s.due_at ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| Ok(StudentVocabulary {
            id:              r.get(0)?,
            vocabulary_id:   r.get(1)?,
            word:            r.get(2)?,
            reading:         r.get(3)?,
            meaning:         r.get(4)?,
            fluency_level:   r.get(5)?,
            times_correct:   r.get(6)?,
            times_incorrect: r.get(7)?,
        }))?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // -----------------------------------------------------------------------
    // Vocabulary scanning
    // -----------------------------------------------------------------------

    /// Returns IDs of vocabulary words from `topic_id` that appear in `text`.
    /// Uses substring matching — correct for Japanese where words have no spaces.
    pub fn find_vocab_in_text(&self, text: &str, topic_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.id, v.word FROM vocabulary v
             JOIN topic_vocabulary tv ON tv.vocabulary_id = v.id
             WHERE tv.topic_id = ?1",
        )?;
        let mut ids = vec![];
        for row in stmt.query_map(params![topic_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })? {
            let (id, word) = row?;
            if text.contains(word.as_str()) {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    // -----------------------------------------------------------------------
    // Topic progress
    // -----------------------------------------------------------------------

    pub fn mark_topic_status(&self, topic_id: i64, status: TopicStatus) -> Result<()> {
        match status {
            TopicStatus::InProgress => {
                self.conn.execute(
                    "INSERT INTO student_topic_progress (topic_id, status, started_at)
                     VALUES (?1, 'in_progress', CURRENT_TIMESTAMP)
                     ON CONFLICT(topic_id) DO UPDATE
                       SET status     = 'in_progress',
                           started_at = COALESCE(started_at, CURRENT_TIMESTAMP)",
                    params![topic_id],
                )?;
            }
            TopicStatus::Completed => {
                self.conn.execute(
                    "INSERT INTO student_topic_progress (topic_id, status, completed_at)
                     VALUES (?1, 'completed', CURRENT_TIMESTAMP)
                     ON CONFLICT(topic_id) DO UPDATE
                       SET status       = 'completed',
                           completed_at = CURRENT_TIMESTAMP",
                    params![topic_id],
                )?;
            }
            TopicStatus::NotStarted => {
                self.conn.execute(
                    "DELETE FROM student_topic_progress WHERE topic_id = ?1",
                    params![topic_id],
                )?;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Lesson summaries
    // -----------------------------------------------------------------------

    pub fn save_lesson_summary(&self, summary: &LessonSummary) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO lesson_summaries (session_id, notes) VALUES (?1, ?2)",
            params![summary.session_id, summary.notes],
        )?;
        let id = self.conn.last_insert_rowid();

        for t in &summary.topics {
            self.conn.execute(
                "INSERT INTO lesson_summary_topics
                 (lesson_summary_id, topic_id, topic_name, status)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, t.topic_id, t.topic_name, t.status],
            )?;
        }
        for v in &summary.vocabulary {
            self.conn.execute(
                "INSERT INTO lesson_summary_vocabulary
                 (lesson_summary_id, vocabulary_id, word, outcome)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, v.vocabulary_id, v.word, v.outcome],
            )?;
        }
        Ok(id)
    }

    pub fn latest_lesson_summary(&self) -> Result<Option<LessonSummary>> {
        let row = self.conn.query_row(
            "SELECT id, session_id, notes
             FROM lesson_summaries ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, String>(2)?,
            )),
        );
        let (id, session_id, notes) = match row {
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(r)  => r,
        };

        let mut stmt = self.conn.prepare(
            "SELECT topic_id, topic_name, status
             FROM lesson_summary_topics WHERE lesson_summary_id = ?1",
        )?;
        let topics: Vec<LessonSummaryTopic> = stmt
            .query_map(params![id], |r| Ok(LessonSummaryTopic {
                topic_id:   r.get(0)?,
                topic_name: r.get(1)?,
                status:     r.get(2)?,
            }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut stmt2 = self.conn.prepare(
            "SELECT vocabulary_id, word, outcome
             FROM lesson_summary_vocabulary WHERE lesson_summary_id = ?1",
        )?;
        let vocabulary: Vec<LessonSummaryVocab> = stmt2
            .query_map(params![id], |r| Ok(LessonSummaryVocab {
                vocabulary_id: r.get(0)?,
                word:          r.get(1)?,
                outcome:       r.get(2)?,
            }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Some(LessonSummary { id, session_id, notes, topics, vocabulary }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn in_memory() -> Db {
        let conn = Connection::open_in_memory().unwrap();
        let db = Db { conn };
        db.migrate().unwrap();
        db
    }

    // ── Learner profile ──────────────────────────────────────────────────────

    #[test]
    fn default_profile_is_n5() {
        let db = in_memory();
        let profile = db.learner_profile().unwrap();
        assert_eq!(profile.current_level, 5);
        assert_eq!(profile.target_level, 4);
        assert_eq!(profile.total_words, 0);
    }

    // ── Sessions ─────────────────────────────────────────────────────────────

    #[test]
    fn start_and_end_session_succeeds() {
        let db = in_memory();
        let id = db.start_session().unwrap();
        assert!(id > 0);
        db.end_session(id).unwrap();
    }

    #[test]
    fn record_turn_stores_input_and_response() {
        let db = in_memory();
        let session_id = db.start_session().unwrap();
        let turn_id = db.record_turn(session_id, "こんにちは", "いい天気ですね", false).unwrap();
        assert!(turn_id > 0);
    }

    // ── Vocabulary ───────────────────────────────────────────────────────────

    #[test]
    fn upsert_vocabulary_returns_stable_id() {
        let db = in_memory();
        let entry = VocabEntry { id: 0, word: "猫".into(), reading: Some("ねこ".into()), meaning: Some("cat".into()), jlpt_level: Some(5) };
        let id1 = db.upsert_vocabulary(&entry).unwrap();
        let id2 = db.upsert_vocabulary(&entry).unwrap();
        assert_eq!(id1, id2, "upsert on duplicate should return same id");
    }

    #[test]
    fn introduce_word_creates_student_vocab_and_srs_entry() {
        let db = in_memory();
        let entry = VocabEntry { id: 0, word: "犬".into(), reading: None, meaning: Some("dog".into()), jlpt_level: Some(5) };
        let vocab_id = db.upsert_vocabulary(&entry).unwrap();
        db.introduce_word(vocab_id).unwrap();

        // student_vocabulary row should exist
        let sv_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM student_vocabulary WHERE vocabulary_id = ?1",
            params![vocab_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(sv_count, 1);

        // srs_schedule row should exist
        let srs_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM srs_schedule WHERE item_type = 'vocabulary' AND item_id = ?1",
            params![vocab_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(srs_count, 1);
    }

    #[test]
    fn introduce_word_is_idempotent() {
        let db = in_memory();
        let entry = VocabEntry { id: 0, word: "魚".into(), reading: None, meaning: Some("fish".into()), jlpt_level: Some(5) };
        let vocab_id = db.upsert_vocabulary(&entry).unwrap();
        db.introduce_word(vocab_id).unwrap();
        db.introduce_word(vocab_id).unwrap(); // second call must not error or duplicate
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM student_vocabulary WHERE vocabulary_id = ?1",
            params![vocab_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    // ── SRS scheduling ───────────────────────────────────────────────────────

    #[test]
    fn newly_introduced_word_is_not_immediately_due() {
        let db = in_memory();
        let entry = VocabEntry { id: 0, word: "山".into(), reading: None, meaning: Some("mountain".into()), jlpt_level: Some(5) };
        let vocab_id = db.upsert_vocabulary(&entry).unwrap();
        db.introduce_word(vocab_id).unwrap();
        let due = db.vocabulary_due_for_review(10).unwrap();
        assert!(due.is_empty(), "word due tomorrow should not appear in today's review");
    }

    #[test]
    fn overdue_word_appears_in_review() {
        let db = in_memory();
        let entry = VocabEntry { id: 0, word: "川".into(), reading: None, meaning: Some("river".into()), jlpt_level: Some(5) };
        let vocab_id = db.upsert_vocabulary(&entry).unwrap();
        db.introduce_word(vocab_id).unwrap();
        // Backdate the due_at so the word is overdue.
        db.conn.execute(
            "UPDATE srs_schedule SET due_at = datetime('now', '-1 day') WHERE item_id = ?1",
            params![vocab_id],
        ).unwrap();
        let due = db.vocabulary_due_for_review(10).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].word, "川");
    }

    #[test]
    fn correct_answer_increases_fluency_and_srs_interval() {
        let db = in_memory();
        let entry = VocabEntry { id: 0, word: "空".into(), reading: None, meaning: Some("sky".into()), jlpt_level: Some(5) };
        let vocab_id = db.upsert_vocabulary(&entry).unwrap();
        db.introduce_word(vocab_id).unwrap();
        db.update_word_fluency(vocab_id, true).unwrap();

        let fluency: u8 = db.conn.query_row(
            "SELECT fluency_level FROM student_vocabulary WHERE vocabulary_id = ?1",
            params![vocab_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(fluency, 1);

        let interval: f64 = db.conn.query_row(
            "SELECT interval_days FROM srs_schedule WHERE item_id = ?1",
            params![vocab_id], |r| r.get(0),
        ).unwrap();
        assert!(interval > 1.0, "interval should increase after correct answer");
    }

    #[test]
    fn incorrect_answer_decreases_fluency_and_resets_interval() {
        let db = in_memory();
        let entry = VocabEntry { id: 0, word: "海".into(), reading: None, meaning: Some("sea".into()), jlpt_level: Some(5) };
        let vocab_id = db.upsert_vocabulary(&entry).unwrap();
        db.introduce_word(vocab_id).unwrap();
        // First get fluency to 2 so a decrease is measurable.
        db.update_word_fluency(vocab_id, true).unwrap();
        db.update_word_fluency(vocab_id, true).unwrap();
        db.update_word_fluency(vocab_id, false).unwrap();

        let fluency: u8 = db.conn.query_row(
            "SELECT fluency_level FROM student_vocabulary WHERE vocabulary_id = ?1",
            params![vocab_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(fluency, 1);

        let interval: f64 = db.conn.query_row(
            "SELECT interval_days FROM srs_schedule WHERE item_id = ?1",
            params![vocab_id], |r| r.get(0),
        ).unwrap();
        assert!((interval - 1.0).abs() < 0.01, "interval should reset to 1 after incorrect answer");
    }

    // ── Vocabulary scanning ──────────────────────────────────────────────────

    #[test]
    fn find_vocab_in_text_matches_words_present_in_topic() {
        let db = in_memory();
        // Grab the first seeded topic id.
        let topic_id: i64 = db.conn
            .query_row("SELECT id FROM topics ORDER BY id LIMIT 1", [], |r| r.get(0))
            .unwrap();
        // Grab one vocabulary word from that topic.
        let word: String = db.conn
            .query_row(
                "SELECT v.word FROM vocabulary v
                 JOIN topic_vocabulary tv ON tv.vocabulary_id = v.id
                 WHERE tv.topic_id = ?1 LIMIT 1",
                params![topic_id],
                |r| r.get(0),
            )
            .unwrap();
        let hits = db.find_vocab_in_text(&word, topic_id).unwrap();
        assert!(!hits.is_empty(), "should find {:?} in its own topic vocabulary", word);
    }

    #[test]
    fn find_vocab_in_text_returns_empty_for_irrelevant_text() {
        let db = in_memory();
        let topic_id: i64 = db.conn
            .query_row("SELECT id FROM topics ORDER BY id LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let hits = db.find_vocab_in_text("xyzzy", topic_id).unwrap();
        assert!(hits.is_empty());
    }

    // ── Lesson summaries ─────────────────────────────────────────────────────

    #[test]
    fn save_and_retrieve_lesson_summary() {
        let db = in_memory();
        let summary = LessonSummary {
            id: 0,
            session_id: None,
            notes: "Practised greetings.".into(),
            topics: vec![],
            vocabulary: vec![],
        };
        db.save_lesson_summary(&summary).unwrap();
        let retrieved = db.latest_lesson_summary().unwrap().unwrap();
        assert_eq!(retrieved.notes, "Practised greetings.");
    }

    #[test]
    fn latest_lesson_summary_returns_none_when_empty() {
        let db = in_memory();
        assert!(db.latest_lesson_summary().unwrap().is_none());
    }
}
