"""
Nihongo sidecar — exposes three endpoints to the Rust host:

  POST /asr/transcribe   — 16kHz mono f32 audio (base64) → transcript text
  POST /llm/chat         — messages[] → SSE stream of TutorResponse JSON
  POST /tts/speak        — text → WAV audio bytes

Run: uvicorn server:app --host 127.0.0.1 --port 8091
"""

import asyncio
import base64
import concurrent.futures
import io
import json
from typing import AsyncGenerator

import numpy as np
from fastapi import FastAPI
from fastapi.responses import Response, StreamingResponse
from pydantic import BaseModel

from models import Models

app = FastAPI()
_models: Models | None = None

# Single-threaded executor for all ML inference — MLX GPU streams are
# thread-local, so keeping everything on one thread avoids stream errors.
_ml_executor = concurrent.futures.ThreadPoolExecutor(max_workers=1)


@app.on_event("startup")
async def startup():
    global _models
    loop = asyncio.get_event_loop()
    _models = await loop.run_in_executor(_ml_executor, Models)


# ---------------------------------------------------------------------------
# Request / response shapes
# ---------------------------------------------------------------------------

class TranscribeRequest(BaseModel):
    audio_b64: str                  # base64-encoded little-endian f32 PCM at 16kHz mono
    language: str | None = None     # None = auto-detect per utterance
    initial_prompt: str | None = None  # steers output language/script without forcing one


class TranscribeResponse(BaseModel):
    transcript: str


class ChatMessage(BaseModel):
    role: str               # "system" | "user" | "assistant"
    content: str


class ChatRequest(BaseModel):
    messages: list[ChatMessage]
    max_tokens: int = 1024


class SpeakRequest(BaseModel):
    text: str
    voice: str = "Ono_Anna"


# ---------------------------------------------------------------------------
# ASR endpoint
# ---------------------------------------------------------------------------

@app.post("/asr/transcribe", response_model=TranscribeResponse)
async def transcribe(req: TranscribeRequest):
    raw = base64.b64decode(req.audio_b64)
    audio = np.frombuffer(raw, dtype=np.float32).copy()
    loop = asyncio.get_event_loop()
    transcript = await loop.run_in_executor(
        _ml_executor, _run_asr, audio, req.language, req.initial_prompt
    )
    return TranscribeResponse(transcript=transcript.strip())


def _run_asr(audio: np.ndarray, language: str | None, initial_prompt: str | None) -> str:
    """Blocking ASR inference — runs on _ml_executor so MLX streams match."""
    import mlx.core as mx
    from mlx_audio.stt.generate import generate_transcription
    asr_model = _models.get_asr()
    audio_mx = mx.array(audio)
    kwargs = {"task": "transcribe"}
    if language is not None:
        kwargs["language"] = language
    if initial_prompt is not None:
        kwargs["initial_prompt"] = initial_prompt
    segments = generate_transcription(model=asr_model, audio=audio_mx, **kwargs)
    if segments is None:
        return ""
    if isinstance(segments, list):
        return "".join(s.get("text", "") if isinstance(s, dict) else getattr(s, "text", str(s)) for s in segments)
    if hasattr(segments, "text"):
        return segments.text
    return str(segments)


# ---------------------------------------------------------------------------
# LLM chat endpoint (SSE stream)
# ---------------------------------------------------------------------------

@app.post("/llm/chat")
async def chat(req: ChatRequest):
    messages = [{"role": m.role, "content": m.content} for m in req.messages]
    return StreamingResponse(
        _stream_chat(messages, req.max_tokens),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


async def _stream_chat(messages: list[dict], max_tokens: int) -> AsyncGenerator[str, None]:
    from mlx_lm import stream_generate

    tokenizer = _models.llm_tokenizer
    model = _models.llm_model

    prompt = tokenizer.apply_chat_template(
        messages,
        tokenize=False,
        add_generation_prompt=True,
        enable_thinking=False,
    )

    loop = asyncio.get_event_loop()
    queue: asyncio.Queue[str | dict | None] = asyncio.Queue()

    def _generate():
        prompt_tokens = len(tokenizer.encode(prompt))
        generation_tokens = 0
        try:
            for response in stream_generate(model, tokenizer, prompt, max_tokens=max_tokens):
                text = response.text if hasattr(response, "text") else str(response)
                queue.put_nowait(text)
                generation_tokens += 1
        finally:
            queue.put_nowait({"prompt_tokens": prompt_tokens, "generation_tokens": generation_tokens})
            queue.put_nowait(None)

    loop.run_in_executor(_ml_executor, _generate)

    while True:
        item = await queue.get()
        if item is None:
            break
        if isinstance(item, dict):
            yield f"data: {json.dumps({'usage': item})}\n\n"
        else:
            yield f"data: {json.dumps({'token': item})}\n\n"

    yield "data: [DONE]\n\n"


# ---------------------------------------------------------------------------
# TTS endpoint
# ---------------------------------------------------------------------------

@app.post("/tts/speak")
async def speak(req: SpeakRequest):
    loop = asyncio.get_event_loop()
    wav_bytes = await loop.run_in_executor(_ml_executor, _run_tts, req.text, req.voice)
    return Response(content=wav_bytes, media_type="audio/wav")


def _run_tts(text: str, voice: str) -> bytes:
    """Blocking TTS inference — runs in a thread executor."""
    import numpy as np
    import soundfile as sf

    tts = _models.get_tts()

    audio_chunks = []
    sample_rate = 24000
    for result in tts.generate(text, voice=voice, temperature=0.0):
        audio_chunks.append(np.array(result.audio))
        sample_rate = result.sample_rate

    if not audio_chunks:
        raise ValueError("TTS produced no audio")

    audio = np.concatenate(audio_chunks)
    buf = io.BytesIO()
    sf.write(buf, audio, sample_rate, format="WAV", subtype="PCM_16")
    return buf.getvalue()


# ---------------------------------------------------------------------------
# Health check
# ---------------------------------------------------------------------------

@app.get("/health")
async def health():
    return {"status": "ok"}
