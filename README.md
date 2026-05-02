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
│   React Frontend (🔄 not yet wired)  src-tauri/src/lib.rs  │
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
    └── db/                     ← SQLite: sessions, turns, vocabulary, SRS, learner profile
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── store.rs        ← Db struct: all read/write operations
            └── types.rs        ← LearnerProfile, LessonSummary, VocabEntry

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
1. cpal captures mic audio at native sample rate (48kHz stereo typical)
2. Resampler converts to 16kHz mono f32
3. VAD detects speech start → audio accumulates in utterance buffer
4. VAD detects trailing silence → utterance complete, send buffered clip to ASR
5. ASR returns transcript text → passed to LLM with conversation context
6. LLM returns structured JSON { transcript, response, milestone }
7. transcript streamed to React (live display)
8. response tokens stream to Qwen3-TTS
9. TTS streams PCM audio back → cpal output stream → speaker
10. VAD monitors mic during TTS playback → barge-in detected → interrupt TTS
11. db crate extracts vocabulary from transcript + response, updates word records
```

### Barge-in

When the user speaks while the tutor is responding:

```
VAD detects speech energy during TTS playback
     ↓
Tauri command cancels current TTS stream
     ↓
Fresh audio chunks flow to ASR
     ↓
LLM responds to the interruption naturally
```

---

## Python Sidecar

The sidecar is a Python process exposing three local HTTP endpoints on `localhost:8091`. It runs Qwen3-ASR and Qwen3-TTS via `mlx-audio` and the tutor LLM via `mlx-lm`. Model paths are read from `Models.json` at startup. Rust communicates with it via `reqwest` streaming calls.

### Endpoints used

| Endpoint | Direction | Purpose |
|---|---|---|
| `POST /asr/transcribe` | Rust → ASR | Send 16kHz mono f32 audio clip, receive transcript text |
| `POST /llm/chat` (stream) | Rust → LLM | Send messages[], receive structured JSON token stream |
| `POST /tts/speak` (WebSocket) | Rust → TTS | Send text tokens, receive PCM frames |

### Audio format into ASR

Complete utterance sent as raw PCM bytes (base64), 16kHz mono f32. The VAD determines the utterance boundary in Rust — the sidecar receives a finished clip, not a stream.

### Text format out of LLM

The LLM is instructed to return structured JSON via mlx-lm's `guided_json` parameter:

```json
{
  "transcript": "What does momo mean?",
  "response": "もも (momo) means peach! It's an N4 word...",
  "milestone": false
}
```

- `transcript` — what the user said (may contain mixed English/Japanese)
- `response` — the tutor's reply, streamed token by token to TTS
- `milestone` — `true` when the tutor considers the exchange closed with positive feedback; triggers context compaction (see Context Management)

Using `guided_json` rather than prompt-engineering a delimiter avoids fragile bracket-splitting, which fails when Japanese text naturally contains 「」 characters.

### TTS

Qwen3-TTS-12Hz-1.7B-VoiceDesign runs in the same sidecar process. Text tokens from the LLM response pipe directly into TTS via WebSocket as they arrive — sentence boundary buffered. First audio packet latency is ~97ms.

---

## Vocabulary Database

SQLite via the `db` crate. Schema designed around JLPT-ordered spaced repetition.

### Tables

```sql
-- Every Japanese word/phrase the user has ever encountered
CREATE TABLE vocabulary (
    id          INTEGER PRIMARY KEY,
    word        TEXT NOT NULL,        -- 食べる
    reading     TEXT,                 -- たべる
    meaning     TEXT,                 -- to eat
    jlpt_level  INTEGER,              -- 5=N5, 4=N4 ... 1=N1, 0=unclassified
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Each time the word appeared in conversation
CREATE TABLE encounters (
    id              INTEGER PRIMARY KEY,
    vocabulary_id   INTEGER REFERENCES vocabulary(id),
    encountered_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    context         TEXT,             -- the sentence it appeared in
    understood      BOOLEAN           -- did the user use/respond correctly?
);

-- SRS scheduling per word
CREATE TABLE srs_schedule (
    vocabulary_id   INTEGER PRIMARY KEY REFERENCES vocabulary(id),
    interval_days   REAL DEFAULT 1,   -- current SRS interval
    ease_factor     REAL DEFAULT 2.5, -- SM-2 ease factor
    due_at          DATETIME,         -- next review due
    streak          INTEGER DEFAULT 0 -- consecutive correct recalls
);

-- User's current JLPT level per domain
CREATE TABLE learner_profile (
    id              INTEGER PRIMARY KEY,
    current_level   INTEGER DEFAULT 5, -- N5 = beginner
    target_level    INTEGER DEFAULT 4,
    total_words     INTEGER DEFAULT 0,
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### JLPT Ordering

Words are introduced in JLPT level order — N5 first, then N4, N3, N2, N1. Within each level, high-frequency words are prioritised. The tutor system prompt is dynamically constructed from the learner's current vocabulary state:

```
System prompt includes:
- Learner's current level (N5/N4/etc.)
- Words they know well (don't need to explain)
- Words due for review today (weave into conversation)
- Words at the boundary of their level (introduce naturally)
```

### Spaced Repetition (SM-2 variant)

Each word in `srs_schedule` follows a modified SM-2 algorithm:

- First encounter → interval: 1 day
- Correct recall → interval × ease_factor, ease_factor += 0.1
- Incorrect/not recalled → interval reset to 1, ease_factor -= 0.2 (min 1.3)
- Words due today are injected into the tutor's context

### Vocabulary Extraction

After each LLM response, the `db` crate parses the transcript and response text for Japanese vocabulary using a lightweight morphological approach (no external dependency — a curated JLPT word list lookup against known tokens). Each word found is:

1. Looked up against the JLPT word list
2. Added to `vocabulary` if new
3. Added to `encounters` with the sentence context
4. `srs_schedule` updated based on whether the user used it correctly

---

## Context Management

The LLM's context window fills within ~20–30 turns once vocabulary injection and conversation history accumulate. Rather than truncating arbitrarily, the app compacts at natural lesson milestones.

### Key point: one sidecar process, new conversation context

The Python sidecar runs as a single persistent process with all three models loaded. A "new session" simply means Rust builds a fresh `messages[]` array seeded from the lesson summary and sends it to the same running sidecar. The model weights stay loaded. The swap is a pointer change in Rust — effectively free.

### Milestone detection

When the tutor returns `"milestone": true` in its JSON response, the exchange is considered closed — the student answered correctly and the tutor gave positive feedback. This is the trigger point.

### Compaction flow

The milestone fires during TTS playback of the positive feedback response, giving a free window of ~2–5 seconds:

```
milestone: true received
  → TTS begins playing positive feedback audio
  → [in parallel]:
       1. flush conversation_turns and SRS updates to SQLite
       2. ask LLM to produce a structured lesson summary
          { topics_covered, words_introduced, words_reviewed, continue_from }
       3. store summary in SQLite (lesson_summaries table)
       4. build new messages[]: system prompt + student_profile + lesson_summary
  → TTS finishes playing
  → swap active messages[] pointer to the new context (atomic, microseconds)
  → old messages[] dropped
```

The student never sees a pause. The handoff is invisible.

### Fallback

If the summary generation is not finished before TTS ends (slow hardware, long summary), the session manager continues on the existing context and retries at the next milestone. Context accumulates a little further but nothing breaks.

### Lesson summary schema

```sql
-- One row per conversation session
CREATE TABLE sessions (
    id                INTEGER PRIMARY KEY,
    started_at        DATETIME DEFAULT CURRENT_TIMESTAMP,
    ended_at          DATETIME,
    lesson_summary_id INTEGER REFERENCES lesson_summaries(id)
);

-- Each student turn + tutor response
-- Written after every exchange, not just at milestone — ensures vocabulary
-- encounters are never lost to a mid-session crash
CREATE TABLE conversation_turns (
    id              INTEGER PRIMARY KEY,
    session_id      INTEGER REFERENCES sessions(id),
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    student_input   TEXT,           -- raw transcript
    tutor_response  TEXT,           -- tutor response text
    milestone       BOOLEAN DEFAULT FALSE
);

-- Generated at each milestone, seeded into the next conversation context
CREATE TABLE lesson_summaries (
    id               INTEGER PRIMARY KEY,
    created_at       DATETIME DEFAULT CURRENT_TIMESTAMP,
    topics_covered   TEXT,   -- JSON array of grammar points / topics
    words_introduced TEXT,   -- JSON array of vocabulary_ids
    words_reviewed   TEXT,   -- JSON array of vocabulary_ids
    continue_from    TEXT    -- free-text hint for the next context window
);
```

---

## Models

| Model | Size | Role | Runs via |
|---|---|---|---|
| Qwen3-ASR | ~1–2GB | Speech → transcript (handles mid-sentence code-switching) | mlx-audio |
| Qwen3.6-27B-4bit | ~14GB | Tutor LLM | mlx-lm |
| Qwen3-TTS-12Hz-1.7B-VoiceDesign | ~2GB | Text → speech | mlx-audio |
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
| TTS | Qwen3-TTS-12Hz-1.7B-VoiceDesign |

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
`llm` crate: `SidecarClient` with `transcribe()`, `chat_stream()` (SSE), and `speak()`. Parses SSE `data:` lines, assembles token stream. `tutor` crate: `TutorSession` tracks message history and token estimate; `build_system_prompt` constructs the system message from learner profile and last lesson summary.

### Step 4 — cpal output ✅
`AudioPlayer` (`audio_engine/player.rs`) opens a cpal output stream backed by a ring buffer. `play_chunk()` pushes f32 PCM. `stop()` sets a barge-in flag that drains the buffer and silences output immediately. `resume()` clears the flag for the next TTS response.

### Step 5 — Tauri commands ✅
`lib.rs` wires everything into three commands: `start_session`, `stop_session`, `barge_in`. The `handle_turn` async function runs the full per-utterance pipeline (ASR → session → LLM stream → persist turn → compact if needed → TTS playback). Events emitted to React: `transcript`, `response_token`, `response_done`, `error`.

### Step 6 — React UI 🔄
`ChatWindow` component exists with message list and input. **Not yet connected to Tauri** — `useChat.ts` still uses hardcoded test messages. `useEvents.ts` and `api.ts` reference old command/event names from an earlier design. Next: wire `transcript`, `response_token`, and `response_done` events into the chat state, and call `start_session` / `stop_session` from the audio button.

### Step 7 — db crate 🔄
Full SQLite schema in place via `store.rs` (sessions, conversation_turns, lesson_summaries, vocabulary, encounters, srs_schedule, learner_profile). Session tracking, turn recording, learner profile, lesson summary save/load, and SRS scheduling all implemented. **Not yet wired**: `upsert_vocabulary` and `record_encounter` exist but `handle_turn` does not yet call them — vocabulary extraction from transcripts is the remaining piece.

### Step 8 — Dynamic system prompt ✅
`tutor/prompt.rs` builds the system message from `LearnerProfile` (current JLPT level, total words) and the latest `LessonSummary` (topics covered, continue-from hint). Called in `TutorSession::new` and again after each context compaction.

### Step 9 — Context compaction ✅
`TutorSession` tracks a rolling token estimate. When a milestone turn crosses 80% of the 4096-token budget, `compact_context` in `lib.rs` streams a structured lesson summary from the LLM, saves it to SQLite, rebuilds the system prompt, and calls `reset_context` to swap in a fresh message list. The swap is a pointer change — no pause in the conversation.

### Step 10 — Polish ⬜
Barge-in UX tuning. VAD silence threshold configuration. Voice selection for TTS.

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

## Future Considerations

- **Pitch accent feedback** — raw audio from the ASR step can be analysed for pronunciation patterns before transcription
- **Handwriting input** — swap the LLM for a vision-capable Qwen3 variant; user draws kanji, model explains it
- **Reading mode** — paste Japanese text, tutor reads it aloud and explains
- **Export** — Anki deck export from the vocabulary database
- **Multiple learners** — learner_profile table already supports this with a user_id