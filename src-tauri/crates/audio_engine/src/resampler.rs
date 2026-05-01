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