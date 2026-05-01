// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// fn main() {

//     nihongo_lib::run()
// }

// Input device: MacBook Pro Microphone
// Default input config: 1 channels, 48000 Hz, F32

use anyhow::{anyhow, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, StreamConfig,
};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

// 5 seconds of audio at 48kHz stereo — more than enough headroom.
// The consumer thread drains this continuously so it won't fill up in practice.
const RING_BUFFER_SAMPLES: usize = 48_000 * 5;

fn main() -> Result<()> {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No input device found"))?;

    println!("Input device: {}", device.name()?);

    // Ask the device what it supports, then pick the default config.
    let supported = device.default_input_config()?;
    println!(
        "Default input config: {:?} channels, {:?} Hz, {:?}",
        supported.channels(),
        supported.sample_rate(),
        supported.sample_format()
    );

    let config: StreamConfig = supported.clone().into();
    let sample_format = supported.sample_format();

    // --- Ringbuf setup ---
    // split() gives us (Producer, Consumer).
    // Producer goes into the cpal audio callback (runs on the audio thread).
    // Consumer goes into our monitoring thread (runs on a normal thread).
    let rb = HeapRb::<f32>::new(RING_BUFFER_SAMPLES);
    let (mut producer, mut consumer) = rb.split();

    // Flag so we can tell the consumer thread to stop cleanly.
    let running = Arc::new(AtomicBool::new(true));
    let running_consumer = running.clone();

    // --- Consumer thread ---
    // This is where your whisper/VAD logic will eventually live.
    // For now it just drains the buffer and prints stats every second.
    let consumer_thread = thread::spawn(move || {
        let mut total_samples: u64 = 0;
        let mut drain_buf = vec![0f32; 4096];

        while running_consumer.load(Ordering::Relaxed) {
            // Pop as many samples as are available into our local drain buffer.
            let n = consumer.pop_slice(&mut drain_buf);
            total_samples += n as u64;

            if n > 0 {
                // Compute a rough RMS level so we can see audio is actually flowing.
                let rms = (drain_buf[..n].iter().map(|s| s * s).sum::<f32>() / n as f32).sqrt();
                println!(
                    "[consumer] drained {} samples | total {} | RMS {:.4}",
                    n, total_samples, rms
                );
            }

            // Don't spin at 100% CPU — sleep a little between drains.
            // In production this becomes: "do I have enough samples to run Whisper?"
            thread::sleep(Duration::from_millis(100));
        }

        println!("[consumer] shutting down, total samples received: {}", total_samples);
    });

    // --- cpal input stream ---
    // The callback MUST be fast and non-blocking. All it does is push samples
    // into the ringbuf. If the buffer is full we drop samples rather than block.
    let stream = match sample_format {
        SampleFormat::F32 => build_input_stream_f32(&device, &config, producer)?,
        // cpal may give us i16 or u16 on some platforms — convert to f32.
        SampleFormat::I16 => build_input_stream_i16(&device, &config, producer)?,
        SampleFormat::U16 => build_input_stream_u16(&device, &config, producer)?,
        other => return Err(anyhow!("Unsupported sample format: {:?}", other)),
    };

    stream.play()?;
    println!("Recording... press Ctrl+C to stop.");

    // Run for 10 seconds then exit cleanly (remove this in your Tauri app —
    // you'll stop via a tauri::command instead).
    thread::sleep(Duration::from_secs(10));

    running.store(false, Ordering::Relaxed);
    drop(stream); // Stops the audio callback.
    consumer_thread.join().ok();

    println!("Done.");
    Ok(())
}

// --- Stream builders per sample format ---
// Each one converts samples to f32 before pushing, so the rest of the
// pipeline always deals with f32 regardless of hardware format.

fn build_input_stream_f32(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer: impl Producer<Item = f32> + Send + 'static,
) -> Result<cpal::Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _info: &cpal::InputCallbackInfo| {
            // push_slice pushes as many as fit; returns how many were pushed.
            // We intentionally ignore leftover samples rather than blocking.
            let pushed = producer.push_slice(data);
            if pushed < data.len() {
                // This fires if the consumer thread is too slow — means the
                // ring buffer filled up. Tune RING_BUFFER_SAMPLES if you see this.
                eprintln!(
                    "[audio cb] ring buffer full — dropped {} samples",
                    data.len() - pushed
                );
            }
        },
        |err| eprintln!("[audio cb] stream error: {}", err),
        None,
    )?;
    Ok(stream)
}

fn build_input_stream_i16(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer: impl Producer<Item = f32> + Send + 'static,
) -> Result<cpal::Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[i16], _info: &cpal::InputCallbackInfo| {
            // Convert i16 → f32 in [-1.0, 1.0]
            let converted: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
            let pushed = producer.push_slice(&converted);
            if pushed < converted.len() {
                eprintln!("[audio cb] ring buffer full — dropped {} samples", converted.len() - pushed);
            }
        },
        |err| eprintln!("[audio cb] stream error: {}", err),
        None,
    )?;
    Ok(stream)
}

fn build_input_stream_u16(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer: impl Producer<Item = f32> + Send + 'static,
) -> Result<cpal::Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _info: &cpal::InputCallbackInfo| {
            // Convert u16 → f32 in [-1.0, 1.0]
            let converted: Vec<f32> = data
                .iter()
                .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                .collect();
            let pushed = producer.push_slice(&converted);
            if pushed < converted.len() {
                eprintln!("[audio cb] ring buffer full — dropped {} samples", converted.len() - pushed);
            }
        },
        |err| eprintln!("[audio cb] stream error: {}", err),
        None,
    )?;
    Ok(stream)
}