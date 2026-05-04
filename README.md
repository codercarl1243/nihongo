# Nihongo Tutor

A local, privacy-first Japanese language tutor that listens, understands, and responds naturally — handling mixed English and Japanese conversation without missing a beat.

---

## Vision

Nihongo Tutor is a conversational AI language tutor built for people learning Japanese. It runs entirely on your machine (M-series Mac), speaks and listens in real time, and adapts to your vocabulary level using the JLPT framework. There is no cloud dependency for the core conversation loop — your voice never leaves your device.

The tutor understands natural mixed-language speech ("How do I use ありがとう in a sentence?"), responds with appropriate Japanese and English, and tracks every word you encounter so it can resurface them at the right intervals for long-term retention.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Tauri App                            │
│                                                             │
│   React Frontend                     src-tauri/src/lib.rs  │
│   ├── ChatWindow                     ├── start/stop_session │
│   ├── Live transcript stream         ├── barge_in command   │
│   ├── Tutor response streaming       ├── handle_turn()      │
│   └── Vocabulary progress UI        └── compact_context()   │
└──────────────┬──────────────────────────┬───────────────────┘
               │                          │
               ▼                          ▼
┌──────────────────────┐    ┌─────────────────────────────────┐
│   Rust Crates        │    │   Python Sidecar                │
│                      │    │                                 │
│   audio_engine/      │───▶│   POST /asr/transcribe          │
│   ├── capture.rs     │    │   f32 PCM → transcript text     │
│   ├── manager.rs     │    │                                 │
│   ├── player.rs      │    │   POST /llm/chat (SSE)          │
│   ├── resampler.rs   │    │   messages[] → token stream     │
│   └── vad.rs         │    │                                 │
│                      │    │   POST /tts/speak               │
│   llm/               │    │   text → WAV bytes              │
│   └── client.rs      │    │                                 │
│                      │◀───│   GET /health                   │
│   tutor/             │    │   localhost:8091                │
│   ├── prompt.rs      │    └─────────────────────────────────┘
│   └── session.rs     │
│                      │
│   db/                │
│   ├── store.rs       │
│   └── types.rs       │
└──────────────────────┘
```

---

## Crate Structure

```
src-tauri/
├── Cargo.toml                  ← workspace root (members: audio_engine, llm, tutor, db)
├── src/
│   ├── main.rs                 ← Tauri bootstrap
│   └── lib.rs                  ← Tauri commands, full turn pipeline, context compaction
└── crates/
    ├── audio_engine/           ← mic capture, resampling, VAD, playback
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── capture.rs      ← cpal input stream → ringbuf
    │       ├── manager.rs      ← AudioManager, EngineConfig; emits partial + turn_end streams
    │       ├── player.rs       ← AudioPlayer: cpal output, barge-in drain
    │       ├── resampler.rs    ← 48kHz stereo → 16kHz mono f32
    │       └── vad.rs          ← Silero VAD, speech detection
    │
    ├── llm/                    ← sidecar HTTP client
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       └── client.rs       ← SidecarClient: transcribe, chat_stream (SSE), speak
    │
    ├── tutor/                  ← conversation state, session manager, system prompt
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── prompt.rs       ← build_system_prompt from LearnerProfile + LessonSummary
    │       ├── session.rs      ← TutorSession: turn history, token counter, compaction trigger
    │       └── types.rs        ← Message, Role, TutorResponse
    │
    └── db/                     ← SQLite: curriculum, sessions, vocabulary, kanji, SRS, profile
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── store.rs        ← Db struct: migration, all read/write operations
            ├── types.rs        ← LearnerProfile, VocabEntry, KanjiEntry, Topic, LessonSummary …
            └── seed.rs         ← N5_SEED_SQL: 14 topics, 130 vocab, 36 kanji, 4 lesson plans

sidecar/                        ← Python process, serves all three model endpoints
├── server.py                   ← FastAPI: /asr/transcribe, /llm/chat (SSE), /tts/speak, /health
├── models.py                   ← lazy-loads ASR, LLM, TTS from Models.json at startup
├── requirements.txt
└── start.sh
```

---

## Data Flow

### Conversation Loop (always running)

```
1.  cpal captures mic audio at native sample rate (48kHz stereo typical)
2.  Resampler converts to 16kHz mono f32
3.  VAD detects speech start → audio accumulates in utterance buffer
4.  VAD detects trailing silence (level-adjusted: 700ms N1/N2 → 1200ms N4/N5) → turn complete
5.  ASR receives clip — no language constraint, auto-detects Japanese or English per utterance
6.  Empty transcript → silently dropped, mic stays open
7.  Transcript emitted to React (transcript event); mic_status { active: false } emitted (UI shows "Thinking…")
8.  LLM receives full message history → streams plain-text reply sentence by sentence
9.  Each sentence sent to TTS as it arrives (pipelined — TTS for sentence N overlaps LLM generating N+1)
10. Mic muted before first TTS chunk plays; WAV decoded and pushed to AudioPlayer
11. On unmute: 400ms echo tail suppression prevents room echo triggering a false turn
12. mic_status { active: true } emitted — back to "Listening"
13. db scans transcript for topic vocabulary → introduce_word for new words
14. Silence threshold adjusted for next turn (500ms for drill prompts, level default otherwise)
```

### Barge-in

When the user speaks while the tutor is responding:

```
VAD (separate barge_vad instance, isolated LSTM state) detects speech during TTS playback
     ↓
barge_in flag set → TTS sentence loop stops, player buffer drained
Speech captured into barge_buffer (not discarded)
     ↓
On unmute: barge_buffer flushed as the next queued turn
     ↓
Full conversation context intact — no turns lost
```

---

## Audio Pipeline Behaviours

**Level-adjusted silence threshold**
The VAD turn-end silence window scales with JLPT level so beginners get more time to retrieve words:
- N1/N2: 700ms
- N3: 900ms
- N4/N5: 1200ms

After a drill prompt ("try saying…", "repeat after me"), the threshold tightens to 500ms for the next turn only, then resets.

**Echo tail suppression**
After TTS playback finishes and the mic unmutes, VAD is suppressed for 400ms to let room echo decay before turn detection resumes. If the user barges in during TTS, the barge buffer is flushed immediately and the suppression window is skipped.

**Kanji level in tutor responses**
The system prompt constrains the script the tutor uses:
- N5/N4: hiragana and katakana only — no kanji
- N3: common everyday kanji (N3 and below), less familiar kanji in hiragana
- N2/N1: full kanji as a native speaker would write

**Partial chunks (speculative transcription — not yet wired)**
`AudioManager` emits a rolling `partial` channel alongside `turn_end` — a snapshot of the growing speech buffer every ~1 second while the user is speaking. Available for speculative ASR before the turn ends; not yet consumed.

---

## Python Sidecar

The sidecar is a Python process exposing three local HTTP endpoints on `localhost:8091`. It runs Qwen3-ASR and Qwen3-TTS (CustomVoice) via `mlx-audio` and the tutor LLM via `mlx-lm`. Model paths are read from `Models.json` at startup. Rust communicates with it via `reqwest` streaming calls. TTS runs at `temperature=0.0` for deterministic output. All ML inference runs on a single-threaded executor (`ThreadPoolExecutor(max_workers=1)`) since MLX GPU streams are thread-local.

### Endpoints used

| Endpoint | Direction | Purpose |
|---|---|---|
| `POST /asr/transcribe` | Rust → ASR | Send 16kHz mono f32 audio (base64), receive transcript text |
| `POST /llm/chat` (SSE) | Rust → LLM | Send messages[], receive plain-text token stream |
| `POST /tts/speak` | Rust → TTS | Send text, receive WAV bytes |
| `GET /health` | Rust → sidecar | Readiness probe before starting a session |

### Audio format into ASR

Complete utterance sent as raw PCM bytes (base64), 16kHz mono f32. The VAD determines the utterance boundary in Rust — the sidecar receives a finished clip, not a stream.

### Text format out of LLM

The LLM returns plain text — the tutor's reply only. No structured JSON wrapper. `parse_response_pub` in `tutor/src/session.rs` trims whitespace; milestone detection is a TODO placeholder that always returns `false` for now.

### TTS

Qwen3-TTS CustomVoice runs in the same sidecar process. The LLM reply is accumulated in full before being sent to TTS (POST returns a complete WAV). Rust decodes the WAV, pushes PCM to `AudioPlayer`, and polls until playback finishes or a barge-in is detected. Mic capture is muted for the duration to prevent echo.

---

## Database

SQLite via the `db` crate. Schema versioned with `PRAGMA user_version` (currently v1). All tables created on first run; N5 seed data inserted automatically.

### Schema

```
┌──────────────────┐     ┌─────────────────────┐
│ learner_profile  │     │ topics               │
│ sessions         │     │ topic_dependencies   │ (dependency graph)
│ conversation_    │     │ topic_vocabulary     │ (topic ↔ vocab join)
│   turns          │     │ topic_kanji          │ (topic ↔ kanji join)
│                  │     │ student_topic_progress│
└──────────────────┘     └─────────────────────┘
       │                         │
       ▼                         ▼
┌──────────────────┐     ┌─────────────────────┐
│ vocabulary       │     │ kanji                │
│ student_         │     │ student_kanji        │
│   vocabulary     │     │ vocabulary_kanji     │ (vocab ↔ kanji join)
└──────────────────┘     └─────────────────────┘
       │                         │
       └────────────┬────────────┘
                    ▼
             ┌──────────────┐
             │ srs_schedule │ (item_type + item_id covers both)
             └──────────────┘

┌──────────────────────────┐   ┌───────────────────┐
│ lesson_summaries         │   │ lesson_plans       │
│ lesson_summary_topics    │   │ lesson_plan_topics │
│ lesson_summary_vocabulary│   └───────────────────┘
└──────────────────────────┘
```

### Key design decisions

**Vocabulary fluency (0–10 per word)**
Each word a student has encountered gets a `student_vocabulary` row with `fluency_level 0–10`. Fluency increases on correct use and decreases on incorrect attempts. A topic is considered complete when every word in it reaches fluency 10. Words are never "forgotten" — they resurface for review as long as fluency < 10.

**Kanji track (Anki-style)**
Kanji are a separate track from vocabulary (`kanji` table, `student_kanji` progress). They are linked back to vocabulary words via `vocabulary_kanji`. Kanji topics are typed `topic_type = 'kanji'`; vocabulary topics use `'vocabulary'`. The same SRS schedule covers both.

**SRS generic over both**
`srs_schedule` uses `(item_type TEXT, item_id INTEGER)` rather than a per-table FK. Covers vocabulary and kanji with one SM-2 implementation.

**Topic dependency graph**
`topic_dependencies` is a many-to-many table: `(topic_id, depends_on_topic_id)`. The tutor finds the next available topic by selecting the lowest-sequence topic whose dependencies are all completed. Topics with no dependencies (Greetings, Self-Introduction, Numbers 1–10) are available from day one.

**Seed data (N5)**
`db/src/seed.rs` contains `N5_SEED_SQL`: 14 topics, 130 vocabulary entries, 36 kanji, all join-table links, and 4 lesson plans. Inserted with `OR IGNORE` on first run.

### Core tables

```sql
-- Curriculum
CREATE TABLE topics (
    id INTEGER PRIMARY KEY, jlpt_level INTEGER NOT NULL,
    sequence_order INTEGER NOT NULL, name TEXT NOT NULL,
    description TEXT NOT NULL, topic_type TEXT NOT NULL  -- 'vocabulary' | 'kanji'
);
CREATE TABLE topic_dependencies (
    topic_id INTEGER NOT NULL, depends_on_topic_id INTEGER NOT NULL,
    PRIMARY KEY (topic_id, depends_on_topic_id)
);

-- Per-word progress
CREATE TABLE student_vocabulary (
    id INTEGER PRIMARY KEY, vocabulary_id INTEGER NOT NULL UNIQUE,
    fluency_level INTEGER NOT NULL DEFAULT 0,  -- 0–10
    times_correct INTEGER NOT NULL DEFAULT 0, times_incorrect INTEGER NOT NULL DEFAULT 0,
    last_seen_at DATETIME
);

-- SRS (covers vocabulary and kanji)
CREATE TABLE srs_schedule (
    id INTEGER PRIMARY KEY, item_type TEXT NOT NULL, item_id INTEGER NOT NULL,
    interval_days REAL NOT NULL DEFAULT 1, ease_factor REAL NOT NULL DEFAULT 2.5,
    due_at DATETIME, streak INTEGER NOT NULL DEFAULT 0,
    UNIQUE(item_type, item_id)
);

-- Lesson summaries (structured, not text blobs)
CREATE TABLE lesson_summaries (
    id INTEGER PRIMARY KEY, session_id INTEGER, created_at DATETIME, notes TEXT NOT NULL DEFAULT ''
);
CREATE TABLE lesson_summary_topics (
    id INTEGER PRIMARY KEY, lesson_summary_id INTEGER NOT NULL,
    topic_id INTEGER, topic_name TEXT NOT NULL, status TEXT NOT NULL
);
CREATE TABLE lesson_summary_vocabulary (
    id INTEGER PRIMARY KEY, lesson_summary_id INTEGER NOT NULL,
    vocabulary_id INTEGER NOT NULL, word TEXT NOT NULL, outcome TEXT NOT NULL
);
```

### Spaced Repetition (SM-2 variant)

- First encounter → scheduled 1 day out
- Correct recall → `interval × ease_factor`, `ease_factor += 0.1` (max 4.0)
- Incorrect → interval reset to 1 day, `ease_factor -= 0.2` (min 1.3)
- Due items surfaced in `session_context()` for injection into the system prompt

---

## Context Management

The LLM's context window fills within ~20–30 turns once vocabulary injection and conversation history accumulate. Rather than truncating arbitrarily, the app compacts at natural lesson milestones.

### Key point: one sidecar process, new conversation context

The Python sidecar runs as a single persistent process with all three models loaded. A "new session" simply means Rust builds a fresh `messages[]` array seeded from the lesson summary and sends it to the same running sidecar. The model weights stay loaded. The swap is a pointer change in Rust — effectively free.

### Milestone detection

`parse_response_pub` currently returns `milestone: false` on every turn — detection is a TODO. When implemented, a milestone will fire when the student correctly uses a target word and the tutor gives positive feedback.

### Compaction flow

When a milestone turn crosses 80% of the 4096-token budget:

```
milestone: true detected (TODO — currently never fires)
  → turn persisted and vocabulary introduced
  → ask LLM to produce {"notes": "..."} lesson summary
  → save summary to lesson_summaries table
  → call db.session_context() for fresh topic + SRS state
  → rebuild system prompt from new SessionContext
  → call reset_context() to swap in a fresh message list
```

The student never sees a pause — the swap happens between TTS playback completing and the next mic capture.

### Lesson summary format

The LLM is asked to produce `{"notes": "..."}` — a freeform paragraph covering what was practised, any errors, and what to continue next session. `parse_summary_pub` extracts the `notes` field and stores it in `lesson_summaries.notes`. The `lesson_summary_topics` and `lesson_summary_vocabulary` join tables exist for future structured tracking.

---

## Models

| Model | Size | Role | Runs via |
|---|---|---|---|
| Qwen3-ASR | ~1–2GB | Speech → transcript (handles mid-sentence code-switching) | mlx-audio |
| Qwen3.6-27B-4bit | ~14GB | Tutor LLM | mlx-lm |
| Qwen3-TTS-12Hz-1.7B-CustomVoice | ~2GB | Text → speech | mlx-audio |
| ~~Whisper~~ (removed) | — | Abandoned: cannot handle mid-sentence language code-switching (e.g. "what does もも mean?") | — |

> All models run locally. No audio or conversation data is sent to any external service.

### Recommended for M4 Pro 48GB

All three models load simultaneously at ~18GB total, leaving ~30GB free for the OS, KV cache, and app. Model paths are configured in `Models.json` — swap to a smaller or larger LLM variant by updating that file, no code changes needed.

---

## Tech Stack

| Layer | Technology |
|---|---|
| App shell | Tauri v2 (Rust + React) |
| Frontend | React + TypeScript |
| Audio capture | cpal 0.17 |
| Ring buffer | ringbuf 0.4 |
| Resampler | rubato 0.16 |
| VAD | voice_activity_detector (Silero V5) |
| HTTP client | reqwest (streaming) |
| Database | SQLite via rusqlite |
| Async runtime | Tokio |
| Model serving | mlx-lm + mlx-audio (Python sidecar) |
| Model config | `Models.json` (swap models without code changes) |
| STT | Qwen3-ASR |
| Tutor LLM | Qwen3.6-27B-4bit |
| TTS | Qwen3-TTS-12Hz-1.7B-CustomVoice |

---

## Build Order

Each step should be independently runnable and testable before moving to the next.

### Step 1 — audio_engine ✅
Mic capture → ringbuf → resampler → 16kHz mono f32 stream. VAD confirms speech detection. `AudioManager` emits two channels: `partial` (rolling chunks while speaking) and `turn_end` (full utterance on silence). `AudioPlayer` handles cpal output and barge-in drain.

### Step 2 — Python sidecar ✅
FastAPI server (`sidecar/server.py`) loading all three models lazily from `Models.json`. Three endpoints working:
- `POST /asr/transcribe` — base64 f32 PCM → transcript text
- `POST /llm/chat` — messages[] → SSE token stream
- `POST /tts/speak` — text → WAV bytes
- `GET /health` — readiness probe

### Step 3 — llm + tutor crates ✅
`llm` crate: `SidecarClient` with `transcribe()`, `chat_stream()` (SSE), and `speak()`. Parses SSE `data:` lines, assembles plain-text token stream. `tutor` crate: `TutorSession` tracks message history and token estimate; `build_system_prompt` takes a `SessionContext` and constructs the system message (level, active topic, pending words, SRS due, last lesson notes).

### Step 4 — cpal output ✅
`AudioPlayer` (`audio_engine/player.rs`) opens a cpal output stream backed by a ring buffer. `play_chunk()` pushes f32 PCM. `stop()` sets a barge-in flag that drains the buffer and silences output immediately. `resume()` clears the flag for the next TTS response.

### Step 5 — Tauri commands ✅
`lib.rs` wires everything into three commands: `start_session`, `stop_session`, `barge_in`. The `handle_turn` async function runs the full per-utterance pipeline: ASR → push user turn → LLM stream → emit `response_done` → persist turn → introduce vocabulary → compact if milestone → TTS playback with mute/unmute. Events emitted to React: `transcript`, `response_done`, `session_ready`, `error`.

### Step 6 — React UI ✅
`ChatWindow` displays live transcript and tutor responses. All Tauri events wired: `transcript`, `response_done`, `mic_status`, `sidecar_status`, `session_ready`, `error`. Zustand store tracks `micActive` and `isThinking`. Mic indicator shows **Listening** (green pulse) / **Thinking…** (blue) / **Speaking** states. Token usage (prompt + generation) displayed per turn. `start_session` / `stop_session` called from the audio button.

### Step 7 — db crate ✅ (schema + introduction) / 🔄 (fluency updates)
Full N5 curriculum schema in place: topics, topic dependencies, vocabulary, kanji, student_vocabulary, student_kanji, SRS schedule, lesson plans, and structured lesson summaries. N5 seed data (14 topics, 130 words, 36 kanji) inserted on first run via `PRAGMA user_version` migration. `session_context()`, `find_vocab_in_text()`, `introduce_word()`, `update_word_fluency()`, `update_kanji_fluency()`, `mark_topic_status()`, `save_lesson_summary()`, and `latest_lesson_summary()` all implemented. `handle_turn` calls `find_vocab_in_text` + `introduce_word` after each turn. **Not yet wired**: `update_word_fluency` — requires milestone detection to know whether an attempt was correct.

### Step 8 — Dynamic system prompt ✅
`tutor/prompt.rs` builds the system message from a `SessionContext`: student level, active topic name + description, first 5 pending words to introduce, already-seen words in the topic, SRS-due words, and previous lesson notes. Called in `TutorSession::new` (which also marks the active topic as `in_progress`) and rebuilt from a fresh `session_context()` call after each context compaction.

### Step 9 — Context compaction ✅
`TutorSession` tracks a rolling token estimate. When a milestone turn crosses 80% of the 4096-token budget, `compact_context` in `lib.rs` streams a structured lesson summary from the LLM, saves it to SQLite, rebuilds the system prompt, and calls `reset_context` to swap in a fresh message list. The swap is a pointer change — no pause in the conversation.

### Step 10 — Polish 🔄
Done: level-adjusted silence thresholds, barge-in audio buffering (speech during TTS is queued not lost), mic status indicator, 400ms echo tail suppression after TTS playback, kanji-level-appropriate script constraints in tutor responses (hiragana-only for N5/N4, N3 kanji for N3, full kanji for N2/N1).

Remaining: voice selection for TTS, milestone detection, fluency update wiring.

---

## Workspace Cargo.toml

```toml
[workspace]
members = [
    ".",
    "crates/audio_engine",
    "crates/llm",
    "crates/tutor",
    "crates/db",
]
resolver = "2"

[workspace.dependencies]
anyhow       = "1"
tokio        = { version = "1", features = ["full"] }
tokio-stream = "0.1"
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
reqwest      = { version = "0.12", features = ["json", "stream"] }
rusqlite     = { version = "0.31", features = ["bundled"] }

[dependencies]
tauri             = { version = "2", features = [] }
tauri-plugin-opener = "2"
audio_engine      = { path = "crates/audio_engine" }
llm               = { path = "crates/llm" }
tutor             = { path = "crates/tutor" }
db                = { path = "crates/db" }
anyhow            = { workspace = true }
tokio             = { workspace = true }
tokio-stream      = { workspace = true }
serde             = { workspace = true }
serde_json        = { workspace = true }
dirs              = "6"
```

---

## macOS Requirements

```bash
# Microphone permission — add to src-tauri/entitlements.plist
com.apple.security.device.audio-input

# Python environment for sidecar
python3 -m venv .venv
source .venv/bin/activate
pip install mlx-lm mlx-audio

# Models are loaded from Models.json — download to ~/llm_models/ from HuggingFace
# Qwen3-ASR:            ~1–2GB
# Qwen3.6-27B-4bit:     ~14GB
# Qwen3-TTS-1.7B:       ~2GB
```

---

## Privacy

- All audio processing is local — nothing sent to external APIs
- Conversation history stays on device in SQLite
- Models run entirely in the Python sidecar on localhost
- No telemetry, no accounts, no subscriptions

---

## Planned Work

### Placement onboarding for returning learners

One-time screen shown on first launch. Four level-select buttons map to JLPT N5–N2. Selecting one writes `current_level` to `learner_profile` and sets a `setup_complete` flag (new column, schema v2 migration). On subsequent launches the flag is read on mount and the screen is skipped.

**DB changes** (`store.rs`): bump `SCHEMA_VERSION` to 2; add `setup_complete INTEGER DEFAULT 0` to `learner_profile`; add `is_setup_complete()`, `mark_setup_complete()`, `update_learner_level()`.

**Tauri commands** (`lib.rs`): `is_setup_complete() → bool`, `set_learner_level(level: u8)`.

**Frontend**: `isSetupComplete()` + `setLearnerLevel(n)` in `api.ts`; `showOnboarding: bool` in Zustand store; new `src/components/onboarding/index.tsx` using existing `Stack` / `Button` primitives; `App.tsx` checks flag on mount and conditionally renders `<OnboardingScreen />`.

**Prompt** (`prompt.rs`): when `current_level < 5`, inject a note telling the tutor to keep pace brisk — the student has prior experience and is reviewing below-level material.

### DB-backed kanji normalisation in transcript display

ASR outputs standard Japanese (kanji/kana mix). Before showing the transcript to the user, intercept each kanji character and check it against `student_kanji` in SQLite. If the kanji is not yet learnt, convert it to hiragana using `pykakasi` (already available in the sidecar Python env). This means the transcript dynamically reflects the student's actual knowledge — no scary unknown kanji — and improves as they progress.

**Sidecar** (`server.py`): add a lightweight `/asr/display` post-processing step (or inline in `/asr/transcribe`) that accepts the raw transcript + learnt kanji list and returns a display-safe version. Use `pykakasi` for kanji→hiragana conversion.

**Rust** (`lib.rs`): after ASR, fetch the learner's known kanji from `db.known_kanji()`, pass to the display normalisation call, emit the result as the `transcript` event.

**DB** (`store.rs`): add `known_kanji() → Vec<String>` — returns the kanji characters the student has encountered (fluency > 0 in `student_kanji`).

---

### Structured compaction output for fluency wiring

Currently `compact_context` asks the LLM for `{"notes": "..."}` freeform text. Extend the compaction prompt to also output a structured vocabulary outcome list so Rust can programmatically call `update_word_fluency`:

```json
{
  "notes": "...",
  "vocab_outcomes": [
    { "word": "元気", "outcome": "correct" },
    { "word": "天気", "outcome": "confused" }
  ]
}
```

**Tutor** (`session.rs`): extend `parse_summary_pub` to extract `vocab_outcomes`.

**Lib** (`lib.rs`): after compaction, iterate outcomes and call `db.update_word_fluency()` for each — this wires up the fluency tracking that is currently unimplemented.

---

### Smaller model variants for lower-spec hardware

The sidecar loads models from `Models.json` — no code changes required to try smaller variants. Candidates worth benchmarking:

- **ASR**: `Qwen3-ASR-0.6B` — significantly lighter than 1.7B, reported first-packet latency under 100ms
- **TTS**: `Qwen3-TTS-0.6B` — designed for low-latency generation (~97ms end-to-end)
- **LLM**: `Qwen3-8B-4bit` or `Qwen3-1.7B` — fits in 4–8GB, suitable for machines with less RAM

Update `Models.json` paths to test; revert if quality is insufficient. No Rust or Python changes needed.

---

### Cross-platform support (Windows / Linux)

The Rust/Tauri core and React UI are already cross-platform. The Mac-specific parts are:
- `com.apple.security.device.audio-input` entitlement (Mac sandbox only)
- The Python sidecar, which uses MLX (Apple Silicon only)

The sidecar's HTTP interface (`/asr/transcribe`, `/llm/chat`, `/tts/speak`) already provides the right abstraction — Rust doesn't care what runs behind it.

**Proposed approach**: maintain the MLX sidecar for Mac and add a second `start_ollama.sh` + alternate `server_ollama.py` for Windows/Linux using:

| Component | Replacement |
|---|---|
| LLM (mlx-lm) | [Ollama](https://ollama.com) with OpenAI-compatible API |
| ASR (mlx-audio) | [faster-whisper](https://github.com/SYSTRAN/faster-whisper) (CUDA/CPU) |
| TTS (mlx-audio) | [Kokoro](https://github.com/hexgrad/kokoro) or [Piper](https://github.com/rhasspy/piper) |

Rust selects which start script to run based on the platform at build time or via a config flag.

---

## Future Considerations

- **Pitch accent feedback** — raw audio from the ASR step can be analysed for pronunciation patterns before transcription
- **Handwriting input** — swap the LLM for a vision-capable Qwen3 variant; user draws kanji, model explains it
- **Reading mode** — paste Japanese text, tutor reads it aloud and explains
- **Export** — Anki deck export from the vocabulary database
- **Multiple learners** — learner_profile table already supports this with a user_id