"""Loads and holds all three models from Models.json at startup."""

import json
import os
from pathlib import Path


def _resolve(path: str) -> str:
    return str(Path(path).expanduser().resolve())


def load_config() -> dict:
    root = Path(__file__).parent.parent
    config_path = root / "Models.json"
    with open(config_path) as f:
        raw = json.load(f)
    return {m["id"]: _resolve(m["path"]) for m in raw["models"]}


class Models:
    def __init__(self):
        paths = load_config()

        self.asr_path = paths["asr"]
        self.llm_path = paths["llm"]
        self.tts_path = paths["tts"]

        print(f"[models] ASR  : {self.asr_path}")
        print(f"[models] LLM  : {self.llm_path}")
        print(f"[models] TTS  : {self.tts_path}")

        print("[models] loading LLM…")
        from mlx_lm import load as lm_load
        self.llm_model, self.llm_tokenizer = lm_load(self.llm_path)
        print("[models] LLM ready")

        print("[models] loading TTS…")
        from mlx_audio.tts.models.kokoro import KokoroModel
        # Qwen3-TTS is loaded the same way as other mlx-audio TTS models
        self._tts_model = None  # lazy-loaded on first use to avoid import errors
        self._tts_path = self.tts_path
        print("[models] TTS will load on first use")

        print("[models] ASR will load on first use")
        self._asr_model = None

    def get_tts(self):
        if self._tts_model is None:
            import mlx_audio
            self._tts_model = mlx_audio.load(self._tts_path)
        return self._tts_model

    def get_asr(self):
        if self._asr_model is None:
            import mlx_audio
            self._asr_model = mlx_audio.load(self.asr_path)
        return self._asr_model
