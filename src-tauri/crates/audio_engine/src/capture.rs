use anyhow::{anyhow, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream, StreamConfig,
};
use ringbuf::{
    traits::{Producer, Split},
    HeapCons, HeapRb,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

const RING_BUFFER_SAMPLES: usize = 48_000 * 5;

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct AudioCapture {
    _stream: Stream,
    pub config: CaptureConfig,
    running: Arc<AtomicBool>,
    /// Hardware input buffer period in milliseconds, measured from the first
    /// `InputCallbackInfo` timestamp. Zero until the first callback fires.
    /// Used by the AEC to calibrate `stream_delay_ms`.
    pub input_latency_ms: Arc<AtomicU32>,
}

impl AudioCapture {
    pub fn start() -> Result<(Self, HeapCons<f32>)> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("No input device found"))?;

        let supported = device.default_input_config()?;

        // Extract what we need before consuming supported into StreamConfig.
        let sample_rate = supported.sample_rate();
        let channels = supported.channels();
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();

        let capture_config = CaptureConfig { sample_rate, channels };

        println!(
            "[capture] {} Hz, {} ch, {:?}",
            sample_rate, channels, sample_format
        );

        let rb = HeapRb::<f32>::new(RING_BUFFER_SAMPLES);
        let (producer, consumer) = rb.split();
        let running = Arc::new(AtomicBool::new(true));
        let input_latency_ms = Arc::new(AtomicU32::new(0));

        let stream = match sample_format {
            SampleFormat::F32 => build_stream_f32(&device, &config, producer, input_latency_ms.clone())?,
            SampleFormat::I16 => build_stream_i16(&device, &config, producer, input_latency_ms.clone())?,
            SampleFormat::U16 => build_stream_u16(&device, &config, producer, input_latency_ms.clone())?,
            other => return Err(anyhow!("Unsupported sample format: {:?}", other)),
        };

        stream.play()?;
        println!("[capture] stream started");

        Ok((Self { _stream: stream, config: capture_config, running, input_latency_ms }, consumer))
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        println!("[capture] stopped");
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Measure the hardware input buffer period from the first `InputCallbackInfo`.
///
/// `callback - capture` = time the data sat in the hardware buffer before we
/// were invoked = input latency. Called on every callback but is a no-op after
/// the first successful measurement (atomically guarded by the stored value).
fn measure_input_latency(info: &cpal::InputCallbackInfo, latency_ms: &Arc<AtomicU32>) {
    if latency_ms.load(Ordering::Relaxed) == 0 {
        if let Some(d) = info.timestamp().callback
                              .duration_since(&info.timestamp().capture) {
            // Clamp to 1ms minimum so a zero-latency report never disables calibration.
            let ms = d.as_millis().max(1) as u32;
            latency_ms.store(ms, Ordering::Relaxed);
        }
    }
}

fn build_stream_f32(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer: impl Producer<Item = f32> + Send + 'static,
    latency_ms: Arc<AtomicU32>,
) -> Result<Stream> {
    Ok(device.build_input_stream(
        config,
        move |data: &[f32], info: &cpal::InputCallbackInfo| {
            measure_input_latency(info, &latency_ms);
            push_or_warn(&mut producer, data);
        },
        |err| eprintln!("[capture] stream error: {}", err),
        None,
    )?)
}

fn build_stream_i16(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer: impl Producer<Item = f32> + Send + 'static,
    latency_ms: Arc<AtomicU32>,
) -> Result<Stream> {
    Ok(device.build_input_stream(
        config,
        move |data: &[i16], info: &cpal::InputCallbackInfo| {
            measure_input_latency(info, &latency_ms);
            let converted: Vec<f32> =
                data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
            push_or_warn(&mut producer, &converted);
        },
        |err| eprintln!("[capture] stream error: {}", err),
        None,
    )?)
}

fn build_stream_u16(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer: impl Producer<Item = f32> + Send + 'static,
    latency_ms: Arc<AtomicU32>,
) -> Result<Stream> {
    Ok(device.build_input_stream(
        config,
        move |data: &[u16], info: &cpal::InputCallbackInfo| {
            measure_input_latency(info, &latency_ms);
            let converted: Vec<f32> = data
                .iter()
                .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                .collect();
            push_or_warn(&mut producer, &converted);
        },
        |err| eprintln!("[capture] stream error: {}", err),
        None,
    )?)
}

#[inline]
fn push_or_warn(producer: &mut impl Producer<Item = f32>, data: &[f32]) {
    let pushed = producer.push_slice(data);
    if pushed < data.len() {
        eprintln!(
            "[capture] ring buffer full — dropped {} samples",
            data.len() - pushed
        );
    }
}