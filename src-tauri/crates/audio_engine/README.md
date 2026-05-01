# 🎙️ Audio Engine

A lightweight, pure-Rust audio processing library. It captures hardware input, resamples it to a consistent 16kHz mono stream, and performs real-time Voice Activity Detection (VAD) to segment speech into "turns."

This crate is designed to be a "producer"—it generates clean audio data and hands it off to external transcribers.

---

## 🛠️ System Requirements

### 1. Microphone Permissions
Since this library accesses raw hardware via `cpal`, the host application must be authorized by the OS.

*   **macOS**: Add this to your `Info.plist`:
    ```xml
    <key>NSMicrophoneUsageDescription</key>
    <string>Microphone access is required for speech recognition.</string>
    ```
*   **Linux**: Requires ALSA development headers (`libasound2-dev`).

### 2. Audio Format
The engine internally standardizes all captured audio to:
*   **Sample Rate**: 16,000 Hz
*   **Channels**: Mono
*   **Bit Depth**: 32-bit Float (`f32`)

---

## 🚀 Quick Start

Add the engine as a local path dependency in your `Cargo.toml`:

```toml
[dependencies]
audio_engine = { path = "../crates/audio_engine" }
```

### Basic Usage

```rust
use audio_engine::AudioManager;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = audio_engine::AudioManager::new();
    
    // Start listening and get the async ReceiverStream
    let mut turn_stream = manager.start()?;

    while let Some(audio_data) = turn_stream.next().await {
        // audio_data is a Vec<f32> containing one complete speech turn
        println!("Turn detected: {} samples", audio_data.len());
    }

    Ok(())
}
```

---

## 🏗️ Internal Pipeline

1.  **Capture (`cpal`)**: Pulls raw samples from the default system input.
2.  **Resampler (`rubato`)**: High-quality FFT-based resampling from hardware rate to 16kHz.
3.  **VAD (`voice_activity_detector`)**: Silero VAD v5 integration to identify speech boundaries.
4.  **Segmentation**: Orchestrates the "Turn End" logic based on a 600ms silence threshold.
