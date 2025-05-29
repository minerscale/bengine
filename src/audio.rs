use std::f64;

use std::sync::mpsc::{self, Receiver, Sender};

use cpal::BufferSize;
use cpal::{
    Sample, SampleFormat, Stream,
    traits::{DeviceTrait, HostTrait},
};
use log::info;

pub struct Audio {
    pub stream: Stream,
    pub parameter_stream: Sender<AudioParameters>,
}

pub struct AudioParameters {
    distance: f64,
}

impl AudioParameters {
    pub fn new(distance: f64) -> Self {
        Self { distance }
    }
}

struct OscillatorParameters {
    position: f64,
    smoothed_inputs: AudioParameters,
    latest_parameters: AudioParameters,
    alpha: f64,
}

impl OscillatorParameters {
    pub fn new(smoothing_constant: f64) -> Self {
        Self {
            position: 0.0,
            smoothed_inputs: AudioParameters::new(100.0),
            latest_parameters: AudioParameters::new(100.0),
            alpha: 1.0 - f64::exp(-(SAMPLE_RATE as f64).recip() / smoothing_constant),
        }
    }

    fn exponential_smoothing(alpha: f64, new_value: f64, previous_smoothed: f64) -> f64 {
        alpha * new_value + (1.0 - alpha) * previous_smoothed
    }

    pub fn update_latest_parameters(&mut self, new_parameters: AudioParameters) {
        self.latest_parameters = new_parameters;
    }

    pub fn update(&mut self) {
        self.smoothed_inputs = AudioParameters {
            distance: Self::exponential_smoothing(
                self.alpha,
                self.latest_parameters.distance,
                self.smoothed_inputs.distance,
            ),
        }
    }
}

// This is where art lives.
fn write_audio<T: Sample + cpal::FromSample<f64>>(
    data: &mut [T],
    parameters: &mut OscillatorParameters,
) {
    for sample in data.iter_mut() {
        parameters.update();

        let d = (parameters.smoothed_inputs.distance + 1.0).recip();

        parameters.position += (f64::consts::TAU * 440.0 * d.powf(1.5)) / SAMPLE_RATE as f64;

        let out = d.powf(2.0) * parameters.position.sin();

        *sample = Sample::from_sample(out);
    }
}

const SAMPLE_RATE: u32 = 48000;
const BUFFER_SIZE_SAMPLES: u32 = 2048;

impl Audio {
    pub fn new() -> Self {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .expect("no output device available");

        let supported_config = device
            .supported_output_configs()
            .expect("error while querying configs")
            .find(|c| {
                matches!(
                    c.sample_format(),
                    SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
                ) && c.channels() == 2
                    && (c.min_sample_rate()..=c.max_sample_rate())
                        .contains(&cpal::SampleRate(SAMPLE_RATE))
                    && match c.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } => {
                            (min..=max).contains(&&BUFFER_SIZE_SAMPLES)
                        }
                        cpal::SupportedBufferSize::Unknown => {
                            panic!("no way to know if buffer size is good")
                        }
                    }
            })
            .expect("no supported config?!")
            .with_sample_rate(cpal::SampleRate(SAMPLE_RATE));

        info!(
            "Audio Information | host: {} | device: {}",
            host.id().name(),
            device.name().unwrap()
        );

        let mut config = supported_config.config();
        config.buffer_size = BufferSize::Fixed(BUFFER_SIZE_SAMPLES);

        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);

        let mut osc_parameters = OscillatorParameters::new(0.1);

        let (tx, rx): (Sender<AudioParameters>, Receiver<AudioParameters>) = mpsc::channel();

        let callback = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            while let Ok(new_parameters) = rx.try_recv() {
                osc_parameters.update_latest_parameters(new_parameters);
            }

            write_audio(data, &mut osc_parameters);
        };

        let stream = device
            .build_output_stream(&config, callback, err_fn, None)
            .unwrap();

        Self {
            stream,
            parameter_stream: tx,
        }
    }
}
