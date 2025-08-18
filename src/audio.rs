use cpal::{
    BufferSize, SampleFormat, Stream,
    traits::StreamTrait,
    traits::{DeviceTrait, HostTrait},
};
use easy_cast::Cast;
use libpd_rs::{Pd, functions::util::calculate_ticks};
use log::info;

#[allow(unused)]
pub struct Audio {
    pub stream: Stream,
}

pub const SAMPLE_RATE: u32 = 48000;
pub const CHANNELS: usize = 2;
const BUFFER_SIZE_SAMPLES: u32 = 2048;

pub type PdEventFn = dyn Fn(&mut Pd) + Send + Sync;

impl Audio {
    #[allow(clippy::unused_self)]
    pub fn process_events(&mut self, pd: &mut Pd, events: &mut Vec<Box<PdEventFn>>) {
        for event in &mut *events {
            event(pd);
        }

        events.clear();
    }

    pub fn new(pd: &mut Pd) -> Self {
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
                ) && c.channels() == cpal::ChannelCount::try_from(CHANNELS).unwrap()
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

        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {err}");

        let ctx = pd.audio_context();

        // Let's evaluate another pd patch.
        // We could have opened a `.pd` file also.
        let patch = include_str!("pd/patch.pd");

        pd.eval_patch(patch).unwrap();

        pd.on_print(|s| println!("{s}")).unwrap();

        let callback = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let ticks = calculate_ticks(2, data.len().cast());

            ctx.receive_messages_from_pd();

            ctx.process_float(ticks, &[], data);
        };

        let stream = device
            .build_output_stream(&config, callback, err_fn, None)
            .unwrap();

        stream.play().unwrap();

        pd.dsp_on().unwrap();

        Self { stream }
    }
}
