//! Acoustic Echo Cancellation using WebRTC's AEC3 pipeline.
//!
//! Create a linked pair with [`create_aec_pair`]:
//! - Keep the [`AecSink`] wherever `play_chunk` is called; push every audio
//!   chunk sent to the speaker so the AEC has a reference signal.
//! - Give the [`AecProcessor`] to the audio capture thread; call
//!   [`AecProcessor::process_in_place`] on every resampled mic chunk before
//!   it reaches the VAD.
//!
//! The two halves share a lock-free ring buffer internally.

use anyhow::Result;
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapCons, HeapProd, HeapRb,
};
use std::sync::Mutex;
use webrtc_audio_processing::Processor;
use webrtc_audio_processing_config::{Config, EchoCanceller};

/// AEC3 frame size: 10 ms at 16 kHz.
const FRAME_SAMPLES: usize = 160;

/// Five seconds of playback reference headroom at 16 kHz.
const REF_BUF_CAPACITY: usize = 16_000 * 5;

// ── AecSink ──────────────────────────────────────────────────────────────────

/// Writer side of the AEC reference buffer.
///
/// Call [`AecSink::push`] with every chunk sent to the speaker so the AEC
/// processor can cancel it from the mic signal.
pub struct AecSink {
    producer: Mutex<HeapProd<f32>>,
}

impl AecSink {
    /// Feed audio that is being sent to the speaker into the reference buffer.
    ///
    /// `source_rate` is the sample rate of `samples` (e.g. 24 000 for TTS).
    /// The samples are automatically downsampled to 16 kHz before buffering.
    pub fn push(&self, samples: &[f32], source_rate: u32) {
        let mono_16k = if source_rate == 16_000 {
            samples.to_vec()
        } else {
            resample_to_16k(samples, source_rate)
        };
        let mut prod = self.producer.lock().unwrap();
        let pushed = prod.push_slice(&mono_16k);
        if pushed < mono_16k.len() {
            eprintln!(
                "[aec] reference buffer full — dropped {} samples",
                mono_16k.len() - pushed
            );
        }
    }
}

// ── AecProcessor ─────────────────────────────────────────────────────────────

/// Capture-side AEC processor. Lives entirely on the audio capture thread.
///
/// Call [`AecProcessor::process_in_place`] on every 16 kHz mono mic chunk
/// before feeding it to the VAD.
pub struct AecProcessor {
    processor: Processor,
    consumer:  HeapCons<f32>,
    /// Mic samples waiting for a full 160-sample frame to accumulate.
    leftover:  Vec<f32>,
}

impl AecProcessor {
    /// Cancel speaker echo from `samples` (16 kHz mono mic audio) in-place.
    ///
    /// Accumulates samples internally until full 160-sample (10 ms) frames are
    /// available. If fewer processed samples are returned than provided, the
    /// remainder has been held over and will appear in the next call's output —
    /// callers should `continue` the loop when the result is empty.
    pub fn process_in_place(&mut self, samples: &mut Vec<f32>) {
        // Prepend any leftover from the previous call.
        let mut buf = std::mem::take(&mut self.leftover);
        buf.extend_from_slice(samples);

        let mut output = Vec::with_capacity(buf.len());
        let mut pos = 0;

        while pos + FRAME_SAMPLES <= buf.len() {
            // Pull the render (speaker) reference for this frame.
            // Zeros are correct when no audio is playing — AEC3 handles it.
            let mut ref_frame = vec![0.0f32; FRAME_SAMPLES];
            self.consumer.pop_slice(&mut ref_frame);

            // Feed render reference first — required ordering for AEC3.
            let mut render = vec![ref_frame];
            self.processor.process_render_frame(&mut render).ok();

            // Process the capture (mic) frame.
            let mut capture = vec![buf[pos..pos + FRAME_SAMPLES].to_vec()];
            self.processor.process_capture_frame(&mut capture).ok();

            output.extend_from_slice(&capture[0]);
            pos += FRAME_SAMPLES;
        }

        // Hold the incomplete tail for the next call.
        self.leftover = buf[pos..].to_vec();
        *samples = output;
    }
}

// ── Constructor ───────────────────────────────────────────────────────────────

/// Create a linked (sink, processor) pair sharing the same reference buffer.
///
/// Returns an error if the underlying WebRTC AEC processor cannot be
/// initialised (should not happen in practice on supported platforms).
pub fn create_aec_pair() -> Result<(AecSink, AecProcessor)> {
    let rb = HeapRb::<f32>::new(REF_BUF_CAPACITY);
    let (producer, consumer) = rb.split();

    let processor = Processor::new(16_000)
        .map_err(|e| anyhow::anyhow!("WebRTC AEC init failed: {:?}", e))?;

    // stream_delay_ms: typical speaker→room→mic round-trip on a laptop is 50–150ms.
    // Providing a starting estimate lets AEC3 converge immediately rather than
    // spending the first several seconds estimating the delay blindly — important
    // for short TTS clips where the audio may finish before AEC3 has converged.
    processor.set_config(Config {
        echo_canceller: Some(EchoCanceller::Full { stream_delay_ms: Some(100) }),
        ..Default::default()
    });

    let sink = AecSink { producer: Mutex::new(producer) };
    let proc = AecProcessor { processor, consumer, leftover: Vec::new() };

    Ok((sink, proc))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Linear-interpolation downsample to 16 kHz. Sufficient for voice; the
/// VAD and ASR only care about speech frequencies (≤ 8 kHz).
fn resample_to_16k(input: &[f32], source_rate: u32) -> Vec<f32> {
    if input.is_empty() {
        return vec![];
    }
    let ratio   = 16_000.0_f64 / source_rate as f64;
    let out_len = (input.len() as f64 * ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx     = src_pos as usize;
        let frac    = (src_pos - idx as f64) as f32;
        let a       = input[idx];
        let b       = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}
