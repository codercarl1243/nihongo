"""
Nihongo sidecar — exposes three endpoints to the Rust host:

  POST /asr/transcribe   — 16kHz mono f32 audio (base64) → transcript text
  POST /llm/chat         — messages[] → SSE stream of TutorResponse JSON
  POST /tts/speak        — text → WAV audio bytes

Run: uvicorn server:app --host 127.0.0.1 --port 8091
"""

import asyncio
import base64
import io
import json
import struct
from typing import AsyncGenerator

import numpy as np
from fastapi import FastAPI, HTTPException
from fastapi.responses import Response, StreamingResponse
from pydantic import BaseModel

from models import Models

app = FastAPI()
_models: Models | None = None


@app.on_event("startup")
async def startup():
    global _models
    _models = Models()


# ---------------------------------------------------------------------------
# Request / response shapes
# ---------------------------------------------------------------------------

class TranscribeRequest(BaseModel):
    audio_b64: str          # base64-encoded little-endian f32 PCM at 16kHz mono


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
    voice: str = "A warm, clear Japanese female tutor voice with a calm and encouraging tone"


# ---------------------------------------------------------------------------
# ASR endpoint
# ---------------------------------------------------------------------------

@app.post("/asr/transcribe", response_model=TranscribeResponse)
async def transcribe(req: TranscribeRequest):
    raw = base64.b64decode(req.audio_b64)
    n = len(raw) // 4
    audio = np.frombuffer(raw, dtype=np.float32).copy()

    asr = _models.get_asr()

    loop = asyncio.get_event_loop()
    transcript = await loop.run_in_executor(None, _run_asr, asr, audio)

    return TranscribeResponse(transcript=transcript.strip())


def _run_asr(asr_model, audio: np.ndarray) -> str:
    """Blocking ASR inference — runs in a thread executor."""
    import mlx.core as mx
    from mlx_audio.stt.generate import generate_transcription
    audio_mx = mx.array(audio)
    segments = generate_transcription(model=asr_model, audio=audio_mx)
    if segments is None:
        return ""
    if isinstance(segments, list):
        return "".join(s.get("text", "") if isinstance(s, dict) else getattr(s, "text", str(s)) for s in segments)
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
    )

    loop = asyncio.get_event_loop()
    queue: asyncio.Queue[str | None] = asyncio.Queue()

    def _generate():
        try:
            for response in stream_generate(model, tokenizer, prompt, max_tokens=max_tokens):
                text = response.text if hasattr(response, "text") else str(response)
                queue.put_nowait(text)
        finally:
            queue.put_nowait(None)

    loop.run_in_executor(None, _generate)

    while True:
        token = await queue.get()
        if token is None:
            break
        yield f"data: {json.dumps({'token': token})}\n\n"

    yield "data: [DONE]\n\n"


# ---------------------------------------------------------------------------
# TTS endpoint
# ---------------------------------------------------------------------------

@app.post("/tts/speak")
async def speak(req: SpeakRequest):
    loop = asyncio.get_event_loop()
    wav_bytes = await loop.run_in_executor(None, _run_tts, req.text, req.voice)
    return Response(content=wav_bytes, media_type="audio/wav")


def _run_tts(text: str, voice: str) -> bytes:
    """Blocking TTS inference — runs in a thread executor."""
    import numpy as np
    import soundfile as sf

    tts = _models.get_tts()

    audio_chunks = []
    sample_rate = 24000
    for result in tts.generate(text, instruct=voice):
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
