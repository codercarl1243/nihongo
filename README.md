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
│   React Frontend                  src-tauri/src/lib.rs      │
│   ├── Waveform indicator          ├── Tauri commands         │
│   ├── Live transcript stream      ├── audio_engine client    │
│   ├── Tutor response stream       └── tutor sidecar client   │
│   └── Vocabulary progress UI                                 │
└──────────────┬──────────────────────────┬───────────────────┘
               │                          │
               ▼                          ▼
┌──────────────────────┐    ┌─────────────────────────────────┐
│   Rust Crates        │    │   Python Sidecar                │
│                      │    │                                 │
│   audio_engine/      │───▶│   Qwen3-Omni (vLLM-Omni)       │
│   ├── capture.rs     │    │   ├── Receives audio chunks      │
│   ├── resampler.rs   │    │   ├── Streams transcript tokens  │
│   └── vad.rs         │    │   ├── Streams tutor response     │
│                      │    │   └── Signals barge-in support   │
│   tutor/             │    │                                 │
│   └── client.rs      │    │   Qwen3-TTS (vLLM-Omni)        │
│                      │    │   ├── Receives streamed text     │
│   db/                │    │   ├── Streams PCM audio back     │
│   ├── vocabulary.rs  │    │   └── 97ms first-packet latency  │
│   ├── jlpt.rs        │    │                                 │
│   └── srs.rs         │◀───│   Exposes OpenAI-compatible API  │
└──────────────────────┘    │   on localhost:8091              │
                            └─────────────────────────────────┘
```

---

## Crate Structure

```
src-tauri/
├── Cargo.toml                  ← workspace root
├── src/
│   ├── main.rs                 ← Tauri bootstrap
│   └── lib.rs                  ← Tauri commands, wires crates together
└── crates/
    ├── audio_engine/           ← mic capture, resampling, barge-in VAD
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── capture.rs      ← cpal input stream → ringbuf
    │       ├── resampler.rs    ← 48kHz stereo → 16kHz mono f32
    │       └── vad.rs          ← Silero VAD, barge-in detection only
    │
    ├── tutor/                  ← sidecar HTTP client, conversation state
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── client.rs       ← streams audio to Omni, receives text+audio
    │       └── conversation.rs ← turn history, learner context
    │
    └── db/                     ← vocabulary tracking, JLPT, SRS
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── vocabulary.rs   ← word records, encounter counts
            ├── jlpt.rs         ← N5→N1 word lists, level assignment
            └── srs.rs          ← spaced repetition scheduling
```

---

## Data Flow

### Conversation Loop (always running)

```
1. cpal captures mic audio at native sample rate (48kHz stereo typical)
2. Resampler converts to 16kHz mono f32
3. 100ms chunks stream continuously to Python sidecar via HTTP
4. Qwen3-Omni receives chunks, understands when user has finished
5. Omni streams text tokens back → Tauri emits to React (live transcript)
6. Omni response tokens stream simultaneously to Qwen3-TTS
7. TTS streams PCM audio back → cpal output stream → speaker
8. VAD monitors mic during playback → barge-in detected → interrupt TTS
9. db crate extracts vocabulary from transcript, updates word records
```

### Barge-in

When the user speaks while the tutor is responding:

```
VAD detects speech energy during TTS playback
     ↓
Tauri command cancels current TTS stream
     ↓
Fresh audio chunks flow to Omni
     ↓
Omni responds to the interruption naturally
```

---

## Python Sidecar

The sidecar exposes a single local HTTP server on `localhost:8091` using vLLM-Omni. Rust communicates with it via `reqwest` streaming calls.

### Endpoints used

| Endpoint | Direction | Purpose |
|---|---|---|
| `POST /v1/chat/completions` (stream) | Rust → Omni | Send audio chunks, receive text tokens |
| `POST /v1/audio/speech/stream` (WebSocket) | Rust → TTS | Send text tokens, receive PCM frames |

### Audio format into Omni

Raw PCM bytes sent as base64 in the chat message content, 16kHz mono f32, 100ms chunks. The sidecar accumulates and feeds to Omni's block-wise encoder.

### Text format out of Omni

Standard OpenAI streaming delta format. The system prompt instructs Omni to structure output as:

```
「ありがとうございます、でも how do I use it?」
That's a great question! ありがとうございます is used when...
```

The quoted block is the transcript of what the user said. Everything after is the tutor response. The Rust client splits on the closing 」 to separate transcript from response.

### TTS

Qwen3-TTS-12Hz-1.7B-CustomVoice runs alongside Omni. Text tokens from Omni's response pipe directly into TTS via WebSocket as they arrive — sentence boundary buffered. First audio packet latency is ~97ms.

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

After each Omni response, the `db` crate parses the transcript and response text for Japanese vocabulary using a lightweight morphological approach (no external dependency — a curated JLPT word list lookup against known tokens). Each word found is:

1. Looked up against the JLPT word list
2. Added to `vocabulary` if new
3. Added to `encounters` with the sentence context
4. `srs_schedule` updated based on whether the user used it correctly

---

## Models

| Model | Size | Role | Runs via |
|---|---|---|---|
| Qwen3-Omni-30B-A3B | ~20GB | STT + tutor LLM | vLLM-Omni |
| Qwen3-TTS-12Hz-1.7B-CustomVoice | ~3.5GB | Text → speech | vLLM-Omni |
| Whisper (optional, removed) | — | Not used | — |

> All models run locally. No audio or conversation data is sent to any external service.

### Recommended for M4 Pro 48GB

The full Qwen3-Omni-30B-A3B model is the recommended choice — it fits comfortably at ~20GB leaving ample headroom. The smaller 7B variant is available if RAM is a concern on other hardware.

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
| LLM serving | vLLM-Omni (Python sidecar) |
| STT + Tutor LLM | Qwen3-Omni |
| TTS | Qwen3-TTS |

---

## Build Order

Each step should be independently runnable and testable before moving to the next.

### Step 1 — audio_engine ✅
Mic capture → ringbuf → resampler → 16kHz mono f32 stream. VAD confirms speech detection. Barge-in signal working.

### Step 2 — Python sidecar
Set up vLLM-Omni serving Qwen3-Omni and Qwen3-TTS. Verify with curl that audio in → text out works. Verify TTS WebSocket streams PCM.

### Step 3 — tutor crate
Rust HTTP client streams audio chunks to sidecar, receives streaming text. Parse transcript vs response from output. Stream text tokens to TTS endpoint, receive PCM back.

### Step 4 — cpal output
Play PCM audio from TTS through cpal output stream. Implement barge-in: VAD fires during playback → cancel TTS stream → restart audio input flow.

### Step 5 — Tauri commands
Wire `audio_engine` and `tutor` into Tauri commands. Emit events to React: `transcript_token`, `response_token`, `audio_chunk`, `barge_in`.

### Step 6 — React UI
Waveform indicator (VAD driven). Live transcript display. Tutor response streaming display. Audio playback from Tauri events.

### Step 7 — db crate
SQLite schema. Vocabulary extraction from transcript. JLPT lookup. SRS scheduling. Learner profile.

### Step 8 — Dynamic system prompt
Build Omni system prompt from db state: learner level, words due for review, boundary words to introduce. Tune conversation to JLPT level.

### Step 9 — Polish
Barge-in UX tuning. VAD sensitivity per learner. Silence threshold configuration. Voice selection for TTS. Conversation history management.

---

## Workspace Cargo.toml

```toml
[workspace]
members = [
    ".",
    "crates/audio_engine",
    "crates/tutor",
    "crates/db",
]
resolver = "2"

[workspace.dependencies]
anyhow      = "1"
tokio       = { version = "1", features = ["full"] }
tokio-stream = "0.1"
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"

[dependencies]
tauri        = { version = "2", features = ["protocol-asset"] }
audio_engine = { path = "crates/audio_engine" }
tutor        = { path = "crates/tutor" }
db           = { path = "crates/db" }
anyhow       = { workspace = true }
tokio        = { workspace = true }
```

---

## macOS Requirements

```bash
# Microphone permission — add to src-tauri/entitlements.plist
com.apple.security.device.audio-input

# Python environment for sidecar
python3 -m venv .venv
source .venv/bin/activate
pip install vllm-omni

# Download models (automatic on first vLLM-Omni run)
# Or manually:
# Qwen3-Omni: ~20GB from HuggingFace
# Qwen3-TTS:  ~3.5GB from HuggingFace
```

---

## Privacy

- All audio processing is local — nothing sent to external APIs
- Conversation history stays on device in SQLite
- Models run entirely in the Python sidecar on localhost
- No telemetry, no accounts, no subscriptions

---

## Future Considerations

- **Handwriting input** — Qwen3-Omni accepts images, so the user could draw kanji on a tablet and ask about it
- **Pitch accent feedback** — Omni can detect pronunciation patterns from raw audio
- **Reading mode** — paste Japanese text, tutor reads it aloud and explains
- **Export** — Anki deck export from the vocabulary database
- **Multiple learners** — learner_profile table already supports this with a user_id