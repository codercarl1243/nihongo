use anyhow::{anyhow, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// 30 seconds of headroom at 48kHz mono (device-rate samples stored after resampling)
const RING_CAPACITY: usize = 48_000 * 30;

/// Plays back PCM audio received as mono f32 slices at an arbitrary source sample rate.
///
/// Call `start()` once, then feed audio via `play_chunk(samples, source_rate)`.
/// The player resamples to the device's native rate and expands mono → N channels.
/// Call `stop()` to cancel mid-playback (barge-in).
pub struct AudioPlayer {
    barge_in:          Arc<AtomicBool>,
    producer:          Option<ringbuf::HeapProd<f32>>,
    device_sample_rate: u32,
    device_channels:   usize,
    _stream:           Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            barge_in:           Arc::new(AtomicBool::new(false)),
            producer:           None,
            device_sample_rate: 48_000,
            device_channels:    1,
            _stream:            None,
        }
    }

    /// Initialise the cpal output stream. Must be called before `play_chunk`.
    pub fn start(&mut self) -> Result<()> {
        let host   = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no output device found"))?;

        let supported = device.default_output_config()?;
        let channels  = supported.channels() as usize;
        let rate      = supported.sample_rate();

        self.device_sample_rate = rate;
        self.device_channels    = channels;

        let config: StreamConfig = supported.into();

        let rb = HeapRb::<f32>::new(RING_CAPACITY);
        let (producer, mut consumer) = rb.split();
        let barge_in = self.barge_in.clone();

        let stream = device.build_output_stream(
            &config,
            move |output: &mut [f32], _| {
                if barge_in.load(Ordering::Relaxed) {
                    consumer.skip(usize::MAX);
                    output.fill(0.0);
                    return;
                }
                // Ring buffer stores mono samples; expand to device channel count.
                let frames = output.len() / channels;
                for i in 0..frames {
                    let s = consumer.try_pop().unwrap_or(0.0);
                    for ch in 0..channels {
                        output[i * channels + ch] = s;
                    }
                }
            },
            |err| eprintln!("[player] output stream error: {err}"),
            None,
        )?;

        stream.play()?;

        self.producer = Some(producer);
        self._stream  = Some(stream);
        self.barge_in.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Feed a chunk of mono f32 audio at `source_rate` Hz into the playback buffer.
    /// Resamples to the device's native rate via linear interpolation.
    pub fn play_chunk(&mut self, chunk: &[f32], source_rate: u32) -> Result<()> {
        let producer = self.producer.as_mut().ok_or_else(|| anyhow!("player not started"))?;

        if source_rate == self.device_sample_rate {
            let pushed = producer.push_slice(chunk);
            if pushed < chunk.len() {
                eprintln!("[player] buffer full — dropped {} samples", chunk.len() - pushed);
            }
        } else {
            let resampled = resample(chunk, source_rate, self.device_sample_rate);
            let pushed = producer.push_slice(&resampled);
            if pushed < resampled.len() {
                eprintln!("[player] buffer full — dropped {} samples", resampled.len() - pushed);
            }
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

    pub fn device_sample_rate(&self) -> u32 {
        self.device_sample_rate
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Linear interpolation resampler — good enough for TTS playback (no aliasing
/// artefacts audible at speech frequencies when upsampling 24kHz → 48kHz).
fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() {
        return vec![];
    }
    let ratio     = to_rate as f64 / from_rate as f64;
    let out_len   = (input.len() as f64 * ratio).ceil() as usize;
    let mut out   = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx     = src_pos as usize;
        let frac    = (src_pos - idx as f64) as f32;
        let a = input[idx];
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}
