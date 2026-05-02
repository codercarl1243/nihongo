use anyhow::{anyhow, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, SampleRate, StreamConfig,
};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const RING_CAPACITY: usize = 48_000 * 5; // 5 seconds headroom

/// Plays back PCM audio received as 16kHz mono f32 slices.
///
/// Call `start()` once, then feed audio via `play_chunk()`.
/// Call `stop()` to cancel mid-playback (barge-in).
pub struct AudioPlayer {
    barge_in: Arc<AtomicBool>,
    producer: Option<ringbuf::HeapProd<f32>>,
    _stream: Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self { barge_in: Arc::new(AtomicBool::new(false)), producer: None, _stream: None }
    }

    /// Initialise the cpal output stream. Must be called before `play_chunk`.
    pub fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no output device found"))?;

        let supported = device.default_output_config()?;
        let rb = HeapRb::<f32>::new(RING_CAPACITY);
        let (producer, mut consumer) = rb.split();

        let barge_in = self.barge_in.clone();

        let config = StreamConfig {
            channels: supported.channels(),
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device.build_output_stream(
            &config,
            move |output: &mut [f32], _| {
                if barge_in.load(Ordering::Relaxed) {
                    // Drain the ring buffer and silence the output
                    consumer.skip(usize::MAX);
                    output.fill(0.0);
                    return;
                }
                let n = consumer.pop_slice(output);
                // If the buffer runs dry, pad with silence
                output[n..].fill(0.0);
            },
            |err| eprintln!("[player] output stream error: {err}"),
            None,
        )?;

        stream.play()?;

        self.producer = Some(producer);
        self._stream = Some(stream);
        self.barge_in.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Feed a chunk of 16kHz mono f32 audio into the playback buffer.
    /// The player resamples to the device's native sample rate if needed.
    pub fn play_chunk(&mut self, chunk: &[f32]) -> Result<()> {
        let producer = self.producer.as_mut().ok_or_else(|| anyhow!("player not started"))?;
        let pushed = producer.push_slice(chunk);
        if pushed < chunk.len() {
            eprintln!("[player] playback buffer full — dropped {} samples", chunk.len() - pushed);
        }
        Ok(())
    }

    /// Signal barge-in: drain the buffer and silence output immediately.
    pub fn stop(&self) {
        self.barge_in.store(true, Ordering::Relaxed);
    }

    /// Resume after a barge-in so the next TTS response can be played.
    pub fn resume(&self) {
        self.barge_in.store(false, Ordering::Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.barge_in.load(Ordering::Relaxed)
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}
