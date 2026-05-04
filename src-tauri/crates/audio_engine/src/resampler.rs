use anyhow::Result;
use rubato::{FftFixedIn, Resampler};
use ringbuf::traits::Consumer;
use ringbuf::HeapCons;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

// How many input frames we hand to the resampler at a time.
// 1024 frames is a good balance — not so small that we call the resampler
// constantly, not so large that we introduce noticeable latency.
const CHUNK_FRAMES: usize = 1024;

/// Drains interleaved f32 samples from the ringbuf consumer, downmixes
/// to mono, resamples to 16 kHz, and returns accumulated 16 kHz mono frames.
///
/// Call this repeatedly from your inference thread. Each call returns however
/// many 16 kHz frames were produced from whatever was in the ring — could be
/// zero if there wasn't enough input yet to fill a full chunk.
pub struct Resampler16k {
    resampler: FftFixedIn<f32>,
    #[allow(dead_code)]
    input_sample_rate: u32,
    channels: usize,
    // Accumulates interleaved samples from the ring until we have a full chunk.
    input_accum: Vec<f32>,
    // Per-channel deinterleaved buffers fed into rubato.
    channel_bufs: Vec<Vec<f32>>,
    // Pre-allocated output buffer — rubato writes into this.
    #[allow(dead_code)]
    output_buf: Vec<Vec<f32>>,
}

impl Resampler16k {
    pub fn new(input_sample_rate: u32, channels: u16) -> Result<Self> {
        let channels = channels as usize;

        if input_sample_rate == TARGET_SAMPLE_RATE && channels == 1 {
            // Nothing to do — but we still construct so the interface is uniform.
            // process() will just pass samples straight through.
        }

        let resampler = FftFixedIn::<f32>::new(
            input_sample_rate as usize,
            TARGET_SAMPLE_RATE as usize,
            CHUNK_FRAMES,
            2,       // sub_chunks — 2 is a sensible default
            channels,
        )?;

        let output_frames = resampler.output_frames_max();

        Ok(Self {
            resampler,
            input_sample_rate,
            channels,
            input_accum: Vec::with_capacity(CHUNK_FRAMES * channels * 2),
            channel_bufs: vec![vec![0f32; CHUNK_FRAMES]; channels],
            output_buf: vec![vec![0f32; output_frames]; channels],
        })
    }

    /// Drain everything available from the consumer, resample in chunk-sized
    /// batches, and return all the resulting 16 kHz mono f32 samples.
    ///
    /// Returns an empty Vec if fewer than CHUNK_FRAMES frames are available.
    pub fn process_available(&mut self, consumer: &mut HeapCons<f32>) -> Result<Vec<f32>> {
        // Pull all available interleaved samples into our accumulator.
        let mut tmp = [0f32; 4096];
        loop {
            let n = consumer.pop_slice(&mut tmp);
            if n == 0 { break; }
            self.input_accum.extend_from_slice(&tmp[..n]);
        }

        let frame_size = self.channels; // interleaved: one frame = N samples
        let frames_needed = CHUNK_FRAMES * frame_size;
        let mut mono_16k_out: Vec<f32> = Vec::new();

        // Process as many complete chunks as we have accumulated.
        while self.input_accum.len() >= frames_needed {
            let chunk: Vec<f32> = self.input_accum.drain(..frames_needed).collect();

            // Deinterleave into per-channel buffers.
            for ch in 0..self.channels {
                for (frame_idx, sample) in self.channel_bufs[ch].iter_mut().enumerate() {
                    *sample = chunk[frame_idx * self.channels + ch];
                }
            }

            // Resample.
            let resampled = self.resampler.process(&self.channel_bufs, None)?;

            // Downmix all channels to mono and append to output.
            let n_out = resampled[0].len();
            for i in 0..n_out {
                let mono = resampled.iter().map(|ch| ch[i]).sum::<f32>()
                    / self.channels as f32;
                mono_16k_out.push(mono);
            }
        }

        Ok(mono_16k_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::{HeapRb, traits::{Producer, Split}};
    use std::f32::consts::PI;

    fn make_resampler(input_hz: u32, channels: u16) -> Resampler16k {
        Resampler16k::new(input_hz, channels).unwrap()
    }

    /// Push `n_frames` of interleaved samples into a fresh ring and return the consumer.
    fn filled_consumer(samples: Vec<f32>) -> ringbuf::HeapCons<f32> {
        let rb = HeapRb::<f32>::new(samples.len().max(1));
        let (mut prod, cons) = rb.split();
        prod.push_slice(&samples);
        cons
    }

    #[test]
    fn output_length_is_proportional_to_ratio() {
        // 48kHz stereo → 16kHz mono: ratio = 1/3.
        // We push 2× CHUNK_FRAMES (2048 stereo frames = 4096 samples) to guarantee
        // at least one full chunk is processed even with rubato's sub_chunks=2 buffering.
        let n_frames = 2048usize;
        let channels = 2u16;
        let input_hz = 48_000u32;
        let samples = vec![0.0f32; n_frames * channels as usize];
        let mut cons = filled_consumer(samples);
        let mut r = make_resampler(input_hz, channels);
        let out = r.process_available(&mut cons).unwrap();
        let expected = (n_frames as f64 * TARGET_SAMPLE_RATE as f64 / input_hz as f64) as usize;
        // Allow ±20 frames for rubato's internal rounding across sub-chunks.
        assert!(
            !out.is_empty(),
            "should produce output for 2× chunk input"
        );
        assert!(
            out.len() <= expected + 20,
            "output ({}) should not exceed expected ({expected}) by more than 20 frames",
            out.len()
        );
        assert!(
            out.len() >= expected / 2,
            "output ({}) should be at least half of expected ({expected})",
            out.len()
        );
    }

    #[test]
    fn fewer_than_chunk_frames_produces_no_output() {
        // CHUNK_FRAMES = 1024; pushing 512 frames should produce nothing yet.
        let samples = vec![0.0f32; 512 * 2]; // 512 stereo frames
        let mut cons = filled_consumer(samples);
        let mut r = make_resampler(48_000, 2);
        let out = r.process_available(&mut cons).unwrap();
        assert!(out.is_empty(), "partial chunk should produce no output");
    }

    #[test]
    fn mono_passthrough_preserves_dc_value() {
        // 16kHz mono in, 16kHz mono out — output should closely match input amplitude.
        let n_frames = 1024usize;
        // DC signal at 0.5 amplitude.
        let samples = vec![0.5f32; n_frames];
        let mut cons = filled_consumer(samples);
        let mut r = make_resampler(16_000, 1);
        let out = r.process_available(&mut cons).unwrap();
        assert!(!out.is_empty());
        // Allow rubato's windowed sinc to deviate slightly at boundaries.
        let mid = out.len() / 4;
        let end = out.len() * 3 / 4;
        for &s in &out[mid..end] {
            assert!((s - 0.5).abs() < 0.05, "DC passthrough deviated: {s}");
        }
    }

    #[test]
    fn downsampled_sine_survives_frequency_check() {
        // Generate a 440 Hz sine at 48kHz stereo and resample to 16kHz.
        // The sine should still be detectable as a non-zero signal after resampling.
        let input_hz = 48_000u32;
        let n_frames = 4096usize;
        let freq = 440.0f32;
        let mut samples = Vec::with_capacity(n_frames * 2);
        for i in 0..n_frames {
            let v = (2.0 * PI * freq * i as f32 / input_hz as f32).sin();
            samples.push(v); // L
            samples.push(v); // R
        }
        let mut cons = filled_consumer(samples);
        let mut r = make_resampler(input_hz, 2);
        let out = r.process_available(&mut cons).unwrap();
        assert!(!out.is_empty());
        let rms = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt();
        assert!(rms > 0.1, "resampled sine should have non-trivial RMS, got {rms}");
    }
}