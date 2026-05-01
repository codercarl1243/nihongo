use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::Sender;

use super::types::AudioBuffer;

pub fn start_input_stream(tx: Sender<AudioBuffer>) -> cpal::Stream {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .expect("No input device available");

    let config = device
        .default_input_config()
        .expect("Failed to get default config");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), tx),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), tx),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), tx),
    };

    stream
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: Sender<AudioBuffer>,
) -> cpal::Stream
where
    T: cpal::Sample,
{
    let channels = config.channels as usize;

    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let buffer: AudioBuffer = data
                    .chunks(channels)
                    .map(|frame| frame[0].to_f32()) // mono
                    .collect();

                let _ = tx.send(buffer);
            },
            move |err| {
                eprintln!("Stream error: {:?}", err);
            },
            None,
        )
        .expect("Failed to build input stream")
}