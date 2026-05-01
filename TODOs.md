- Store the generated "Speaker Embedding" (the unique voice fingerprint) in a local SQLite database (using the tauri-plugin-sql). That way, when the user "saves" a voice they designed, you just reload that binary blob next time.


You are a senior coder. You love to help and are great at communicating but prefer keeping the information short and succinct.

Your job is to assist planning an application and  break down problems into small bite sized tasks that a junior coder who will be reading documentation or asking for assistance constantly can complete. Any task that is broken down should be scaffolded with pseudo code.



## final app
Tech Stack: Rust + Tauri (backend/desktop), React (frontend)
Core Purpose: Japanese language learning app via streaming STT/TTS through LLMs
Key Components:
STT: Whisper (streaming)
TTS: Qwen3-TTS (or similar)
LLM 1: Fugaku (main conversation/teaching engine)
LLM 2: Context manager (summarizes last 2-3 mins, tracks covered words, user performance)
Database: SQLite (stores JLPT levels, word lists, progress)
Rust Logic: Controls LLM behavior (adjusts Japanese ratio based on user fluency/JLPT level)
Frontend: React UI for chat, controls, progress tracking
Constraints/Requirements:
Streaming audio (STT/TTS)
Two LLMs working in tandem
SQLite for JLPT word management
Rust handles orchestration/policy (Japanese ratio, level tracking)
Context window management (2-3 min buffer + summary from LLM 2)
Structured for JLPT levels (N5-N1)

## Phase 1 — “It talks back” (YOU ARE HERE)
Voice → Whisper → LLM → TTS → Voice

Stack:
STT → Whisper (file-based, not streaming)
LLM → ONE model (Fugaku or Qwen)
TTS → system voice (not Qwen3-TTS yet)
No DB
No second LLM
No JLPT logic

Goal:
You say something → it replies in Japanese

## Phase 2 — “It feels alive”

streaming LLM tokens
better TTS (Qwen3-TTS or Kokoro)
interrupt / cancel speech
smoother UX

Still:

❌ no dual LLM
❌ no JLPT system

## Phase 3 — “It teaches”

JLPT tagging
vocabulary extraction
SQLite storage
spaced repetition

## Phase 4 — “It adapts”

second LLM (context manager)
summarisation
fluency tracking
Japanese ratio control

## current notes
The config.sample_rate() and config.channels() you see printed are what cpal negotiated with your OS — it'll likely be 44100 or 48000 Hz and may be stereo. You'll need both values in step 2 because Whisper wants 16000 Hz mono, so that's exactly what the resampler needs to know.

// Input device: MacBook Pro Microphone
// Default input config: 1 channels, 48000 Hz, F32

The consumer thread's thread::sleep(Duration::from_millis(100)) is the drain cadence — in step 4 this becomes "do I have enough samples accumulated to run a Whisper window?", and you'll tune this interval to match your sliding window size.

The eprintln! about dropped samples is your canary. If you ever see it in production, your inference thread is too slow and you need to either increase the buffer size or reduce Whisper model size.

mic
↓
ringbuf
↓
resampler (16kHz mono)
↓
rolling buffer (always collecting)
↓
VAD + timing
↓
[when turn ends]
↓
send full chunk → whisper
↓
text
↓
LLM judge

audio_engine/
  capture.rs          ← mic → ringbuf
  resampler.rs        ← → 16kHz mono
  vad.rs              ← speech vs silence (pure signal)

pipeline/             ← 🔴 THIS is where turn detection belongs
  rolling_buffer.rs
  turn_detector.rs

transcriber/
  whisper.rs          ← Vec<f32> → String

tutor/
  turn.rs             ← LLM reasoning
  conversation.rs
  tts.rs