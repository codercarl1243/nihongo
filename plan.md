# Nihongo Tutor App — Implementation Guide

## Architecture

```
React UI (Tauri WebView)
    ↕ Tauri commands + streamed events
Rust Backend (src-tauri/)
    ├── Audio layer       cpal (capture) + tts crate (playback)
    ├── STT               whisper-rs  →  Japanese/English text
    ├── Conversation LLM  llama-cpp-2 (Fugaku-LLM-13B Q4_K_M)
    ├── Bookkeeping LLM   llama-cpp-2 (Qwen2.5-1.5B Q4_K_M)
    └── Data              rusqlite (vocabulary, sessions, lesson plan)
```

## Models

| Role | Model | Format | Approx Size |
|------|-------|--------|-------------|
| STT | `ggml-large-v3-turbo` | ggml bin | ~1.6 GB |
| Conversation | `Fugaku-LLM-13B-instruct` Q4_K_M | GGUF | ~7.5 GB |
| Bookkeeping | `Qwen2.5-1.5B-Instruct` Q4_K_M | GGUF | ~1.0 GB |
| TTS | macOS AVSpeechSynthesizer | system | 0 GB |

**Total disk ~10 GB. RAM when running ~9–10 GB.**

Models stored at: `~/Library/Application Support/nihongo/models/`

| File | Source |
|------|--------|
| `fugaku-13b-q4_k_m.gguf` | [mmnga/Fugaku-LLM-13B-instruct-gguf](https://huggingface.co/mmnga/Fugaku-LLM-13B-instruct-gguf) |
| `qwen2.5-1.5b-instruct-q4_k_m.gguf` | [Qwen/Qwen2.5-1.5B-Instruct-GGUF](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF) |
| `ggml-large-v3-turbo.bin` | [ggerganov/whisper.cpp on HuggingFace](https://huggingface.co/ggerganov/whisper.cpp) |

## Conversation Flow (per turn)

```
1. User holds button → cpal records PCM audio
2. Release → whisper-rs transcribes → Japanese or English text
3. Text + history → Fugaku → tokens streamed via Tauri events to React UI
4. Full Fugaku response → tts crate → AVSpeechSynthesizer plays back
5. Async (non-blocking): Fugaku response → Qwen2.5-1.5B
       prompt: extract vocabulary as JSON [{word, reading, meaning_en, jlpt_level}]
       → parse JSON → upsert into SQLite vocabulary table
6. Spaced repetition scheduler updates next_review dates
```

---

## Build Order

1. Add Rust crate dependencies, run `cargo check`
2. SQLite data layer (`db.rs`)
3. Audio capture (`audio.rs`)
4. Whisper STT (`stt.rs`)
5. Fugaku LLM with token streaming (`llm.rs`)
6. Wire Tauri commands in `lib.rs`
7. TTS playback (`tts` crate)
8. Bookkeeping LLM — Qwen vocab extraction → SQLite
9. React UI components
10. First-run model download UI

---

## Step 1 — Rust Dependencies

`src-tauri/Cargo.toml` under `[dependencies]`:

```toml
llama-cpp-2 = { version = "0.1", features = ["metal"] }
whisper-rs  = { version = "0.13", features = ["metal"] }
tts         = "0.26"
cpal        = "0.15"
rusqlite    = { version = "0.31", features = ["bundled"] }
dirs        = "5"
tokio       = { version = "1", features = ["full"] }
```

> Verify the metal features compile before going further: `cd src-tauri && cargo check`

---

## Step 2 — SQLite Data Layer

**`src-tauri/src/db.rs`**

```rust
use rusqlite::{Connection, Result};

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS vocabulary (
            id              INTEGER PRIMARY KEY,
            word            TEXT NOT NULL,
            reading         TEXT,
            meaning_en      TEXT,
            jlpt_level      TEXT,
            encounter_count INTEGER DEFAULT 1,
            next_review     INTEGER,
            ease_factor     REAL DEFAULT 2.5
        );
        CREATE TABLE IF NOT EXISTS sessions (
            id         INTEGER PRIMARY KEY,
            started_at INTEGER,
            ended_at   INTEGER,
            turn_count INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS turns (
            id         INTEGER PRIMARY KEY,
            session_id INTEGER REFERENCES sessions(id),
            role       TEXT CHECK(role IN ('user','assistant')),
            content    TEXT,
            created_at INTEGER
        );
    ")
}
```

**`src-tauri/src/lib.rs`** — app state:

```rust
use rusqlite::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Connection>,
}
```

---

## Step 3 — Model Path Helpers

**`src-tauri/src/models.rs`**

```rust
use std::path::PathBuf;

pub fn models_dir() -> PathBuf {
    dirs::data_dir()
        .expect("no data dir")
        .join("nihongo")
        .join("models")
}

pub fn fugaku_path() -> PathBuf {
    models_dir().join("fugaku-13b-q4_k_m.gguf")
}

pub fn qwen_path() -> PathBuf {
    models_dir().join("qwen2.5-1.5b-instruct-q4_k_m.gguf")
}

pub fn whisper_path() -> PathBuf {
    models_dir().join("ggml-large-v3-turbo.bin")
}
```

---

## Step 4 — Audio Capture

**`src-tauri/src/audio.rs`**

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub async fn record(stop: oneshot::Receiver<()>) -> Vec<f32> {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    let config = device.default_input_config().unwrap();

    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let samples_clone = Arc::clone(&samples);

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _| {
            samples_clone.lock().unwrap().extend_from_slice(data);
        },
        |err| eprintln!("audio error: {err}"),
        None,
    ).unwrap();

    stream.play().unwrap();
    let _ = stop.await;
    drop(stream);

    Arc::try_unwrap(samples).unwrap().into_inner().unwrap()
}
```

Tauri state holds a `Option<oneshot::Sender<()>>` so `stop_recording` can signal the capture to stop.

---

## Step 5 — Whisper STT

**`src-tauri/src/stt.rs`**

```rust
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub fn transcribe(samples: &[f32], model_path: &str) -> String {
    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .expect("failed to load whisper model");
    let mut state = ctx.create_state().unwrap();

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("ja"));
    params.set_print_special(false);
    params.set_print_progress(false);

    state.full(params, samples).unwrap();

    (0..state.full_n_segments().unwrap())
        .map(|i| state.full_get_segment_text(i).unwrap())
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
}
```

---

## Step 6 — Fugaku LLM with Token Streaming

**`src-tauri/src/llm.rs`**

```rust
use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    model::{params::LlamaModelParams, LlamaModel},
};

pub struct LlamaState {
    pub backend: LlamaBackend,
    pub fugaku: LlamaModel,
    pub qwen:   LlamaModel,
}

impl LlamaState {
    pub fn load(fugaku_path: &str, qwen_path: &str) -> Self {
        let backend = LlamaBackend::init().unwrap();
        let params  = LlamaModelParams::default();
        let fugaku  = LlamaModel::load_from_file(&backend, fugaku_path, &params).unwrap();
        let qwen    = LlamaModel::load_from_file(&backend, qwen_path, &params).unwrap();
        Self { backend, fugaku, qwen }
    }
}
```

The `chat` Tauri command builds the prompt (system prompt + conversation history + new user message), then iterates the token stream and emits each token as a Tauri event:

```rust
#[tauri::command]
async fn chat(
    message: String,
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let system_prompt = include_str!("../../prompts/japanese-teacher.txt");
    // build full prompt string from system_prompt + history + message
    // iterate llama-cpp-2 completions:
    //   for each token string → window.emit("token", &token)
    // collect full response and return it
    Ok(full_response)
}
```

---

## Step 7 — TTS Playback

```rust
use tts::Tts;

#[tauri::command]
fn speak(text: String) -> Result<(), String> {
    let mut tts = Tts::default().map_err(|e| e.to_string())?;

    // Pick Kyoko (female) or Otoya (male) Japanese voice
    if let Ok(voices) = tts.voices() {
        if let Some(v) = voices.iter().find(|v| v.name().contains("Kyoko")) {
            let _ = tts.set_voice(v);
        }
    }

    tts.speak(text, true).map_err(|e| e.to_string())?;
    Ok(())
}
```

---

## Step 8 — Bookkeeping LLM (Qwen2.5-1.5B)

After each Fugaku response, spawn a background task using the already-loaded Qwen model:

**Prompt template:**
```
Extract vocabulary from this Japanese text.
Return ONLY a JSON array, no explanation.
Format: [{"word":"","reading":"","meaning_en":"","jlpt_level":""}]

Text: {fugaku_response}
```

Parse the JSON and upsert into SQLite:

```rust
fn upsert_vocabulary(conn: &Connection, words: &[VocabEntry]) -> rusqlite::Result<()> {
    for w in words {
        conn.execute(
            "INSERT INTO vocabulary (word, reading, meaning_en, jlpt_level)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(word) DO UPDATE SET
               encounter_count = encounter_count + 1",
            (&w.word, &w.reading, &w.meaning_en, &w.jlpt_level),
        )?;
    }
    Ok(())
}
```

Add `UNIQUE` constraint on `word` in the schema for the `ON CONFLICT` to work.

---

## Step 9 — lib.rs (wiring everything)

```rust
pub fn run() {
    let db_path = dirs::data_dir().unwrap().join("nihongo").join("nihongo.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(&db_path).unwrap();
    db::init(&db).unwrap();

    let llama = llm::LlamaState::load(
        models::fugaku_path().to_str().unwrap(),
        models::qwen_path().to_str().unwrap(),
    );

    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(db),
            llama: Mutex::new(llama),
            recording_stop: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            chat,
            speak,
            get_vocabulary,
            get_session_history,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
```

---

## Step 10 — React Frontend

### File structure

```
src/
  components/
    VoiceButton.tsx      hold-to-record, pulsing ring while active
    TranscriptPane.tsx   renders streaming tokens as they arrive
    VocabPanel.tsx       vocabulary list with SR due dates
  hooks/
    useAudio.ts          invoke start_recording / stop_recording
    useChat.ts           invoke chat + listen to token events
  App.tsx
```

### useChat.ts

```ts
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';

export function useChat() {
  const [response, setResponse] = useState('');
  const [thinking, setThinking] = useState(false);

  const send = async (message: string) => {
    setResponse('');
    setThinking(true);
    const unlisten = await listen<string>('token', (e) => {
      setResponse(prev => prev + e.payload);
    });
    await invoke('chat', { message });
    unlisten();
    setThinking(false);
  };

  return { response, thinking, send };
}
```

### useAudio.ts

```ts
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';

export function useAudio() {
  const [recording, setRecording] = useState(false);

  const start = () => {
    invoke('start_recording');
    setRecording(true);
  };

  const stop = async (): Promise<string> => {
    const result = await invoke<{ text: string }>('stop_recording');
    setRecording(false);
    return result.text;
  };

  return { recording, start, stop };
}
```

### VoiceButton.tsx

```tsx
import { useAudio } from '../hooks/useAudio';
import { useChat } from '../hooks/useChat';

export function VoiceButton() {
  const { recording, start, stop } = useAudio();
  const { send } = useChat();

  const handlePointerDown = () => start();
  const handlePointerUp = async () => {
    const text = await stop();
    if (text) send(text);
  };

  return (
    <button
      onPointerDown={handlePointerDown}
      onPointerUp={handlePointerUp}
      style={{ background: recording ? 'red' : 'blue' }}
    >
      {recording ? '録音中…' : '話す'}
    </button>
  );
}
```

---

## Tauri Capabilities

`src-tauri/capabilities/default.json` — ensure these permissions are present for microphone access and file system access to the models directory:

```json
{
  "permissions": [
    "core:default",
    "opener:default"
  ]
}
```

You will also need to add `NSMicrophoneUsageDescription` to `src-tauri/Info.plist` (create it if absent) for macOS mic permission.

---

## SM-2 Spaced Repetition (vocabulary review)

A minimal implementation to schedule `next_review`:

```rust
pub fn sm2_update(ease: f32, quality: u8) -> (f32, u32) {
    // quality: 0-5 (5 = perfect recall)
    let new_ease = (ease + 0.1 - (5 - quality) as f32 * (0.08 + (5 - quality) as f32 * 0.02))
        .max(1.3);
    let interval = if quality < 3 { 1 } else { (ease * 6.0) as u32 };
    (new_ease, interval) // interval in days
}
```

Store `next_review` as `UNIXEPOCH('now') + interval * 86400` seconds.

---

## Notes

- Load both LLM models at startup. The first run will be slow (~30s on M-series with 13B). Show a loading screen.
- The `tts` crate blocks until speech finishes. Run it in a `tokio::task::spawn_blocking` to avoid blocking the async runtime.
- whisper-rs requires 16kHz mono f32 samples. Resample from whatever `cpal` gives you if it differs.
- `include_str!("../../prompts/japanese-teacher.txt")` embeds the system prompt at compile time so it's always bundled.
