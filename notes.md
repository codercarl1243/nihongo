# Nihongo

## flow
1. Whisper (speech → text)
2. Qwen Instruct (text → reasoning → response)
3. Qwen3-TTS (response → speech)

## requirements

### Whisper - Speech to Text
- brew install whisper-cpp
-  download model for whisper to work = curl -LO https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

#### debugging and ability to create .wav files
brew install sox
sox -d test.wav

### mlx - Cli to talk with LLMs
pip install mlx-lm
mlx_lm - [github](https://chatgpt.com/c/69f018ec-2d9c-8321-9fd2-bfeb666d69a2)

mlx_lm.chat --model [model address] --prompt "words"
mlx_lm.server --model [model address]

mlx_lm.chat --model ~/llm_models/mlx-community/Qwen3-TTS-12Hz-1.7B-VoiceDesign-bf16 --prompt "こんにちは。簡単な日本語で返 事してください。"

## whisper commands
whisper-cli --help
eg. 

## Qwen3 -TTS commands
python3 -m mlx_audio.tts.generate --model ~/llm_models/mlx-community/Qwen3-TTS-12Hz-1.7B-VoiceDesign-bf16 --text "Hello, this is a test."

whisper-cli -m ~/llm_models/ggml-base.bin -f [filepath]# nihongo
