"""Loads and holds all three models from Models.json at startup."""

import json
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
        from mlx_lm import load as lm_load, stream_generate
        self.llm_model, self.llm_tokenizer = lm_load(self.llm_path)
        print("[models] LLM loaded — warming up MLX kernels…")
        for _ in stream_generate(self.llm_model, self.llm_tokenizer, "hi", max_tokens=1):
            break
        print("[models] LLM ready")

        # ASR and TTS are lazy-loaded on first use
        self._asr_model = None
        self._tts_model = None
        print("[models] ASR + TTS will load on first use")

    def get_asr(self):
        if self._asr_model is None:
            from mlx_audio.stt import load
            self._asr_model = load(self.asr_path)
            print("[models] ASR ready")
        return self._asr_model

    def get_tts(self):
        if self._tts_model is None:
            from mlx_audio.tts import load
            self._tts_model = load(self.tts_path)
            print("[models] TTS ready")
        return self._tts_model
