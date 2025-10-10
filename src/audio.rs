use cpal::{
    BufferSize, SampleFormat,
    traits::StreamTrait,
    traits::{DeviceTrait, HostTrait},
};
use easy_cast::Cast;
use libpd_rs::Pd;
use log::{info, warn};
use notify::Watcher;

use std::{
    io::Read,
    path::Path,
    sync::mpsc::{Receiver, channel},
    time::Duration,
};

#[allow(unused)]
pub struct Audio {
    pub pd_patch_watcher: notify::RecommendedWatcher,
    pub pd_patch_rx: Receiver<Result<notify::Event, notify::Error>>,
    pub pd_patch_path: &'static Path,
}

pub const SAMPLE_RATE: u32 = 48000;
pub const CHANNELS: usize = 2;
const BUFFER_SIZE_SAMPLES: u32 = 1024;

impl Audio {
    pub fn process_events(&mut self, pd: &mut Pd) {
        let mut reload = false;
        while let Ok(event) = self.pd_patch_rx.try_recv() {
            match event {
                Ok(event) => match event.kind {
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                        reload = true;
                    }
                    _ => (),
                },
                Err(e) => warn!("pd patch watch error: {e:?}"),
            }
        }

        if !reload {
            return;
        }

        let mut patch = String::new();
        std::fs::File::open(self.pd_patch_path)
            .unwrap()
            .read_to_string(&mut patch)
            .unwrap();

        if !patch.is_empty() {
            info!("hot reloaded {}", self.pd_patch_path.to_str().unwrap());
            if pd.eval_patch(&patch).is_err() {
                warn!("pd patch error: {patch}");
            }
        }
    }

    pub fn new(pd: &mut Pd) -> Self {
        let (tx, pd_patch_rx) = channel();

        let mut pd_patch_watcher = notify::RecommendedWatcher::new(
            tx,
            notify::Config::default()
                .with_poll_interval(Duration::from_secs(1))
                .with_compare_contents(true),
        )
        .unwrap();

        let pd_patch_path = {
            let first_search_path = Path::new("src/pd/patch.pd");
            let second_search_path = Path::new("patch.pd");

            if std::fs::exists(first_search_path).unwrap_or(false) {
                Some(first_search_path)
            } else if std::fs::exists(second_search_path).unwrap_or(false) {
                Some(second_search_path)
            } else {
                None
            }
        }
        .expect("please place a puredata patch named 'patch.pd' in either ./src/pd/ or .");

        pd_patch_watcher
            .watch(pd_patch_path, notify::RecursiveMode::NonRecursive)
            .unwrap();

        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .expect("no output device available");

        let supported_config = device
            .supported_output_configs()
            .expect("error while querying configs")
            .find(|c| {
                matches!(c.sample_format(), SampleFormat::F32)
                    && c.channels() == cpal::ChannelCount::try_from(CHANNELS).unwrap()
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

        assert_eq!(supported_config.sample_format(), SampleFormat::F32);

        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {err}");

        let ctx = pd.audio_context();

        let mut patch = String::new();
        std::fs::File::open(pd_patch_path)
            .unwrap()
            .read_to_string(&mut patch)
            .unwrap();

        if pd.eval_patch(&patch).is_err() {
            warn!("pd patch error: {patch}");
        }

        pd.on_print(|s| println!("{s}")).unwrap();

        let mut leftovers: Vec<f32> = vec![];
        let mut dst: Vec<f32> = vec![];
        let callback = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let start_point = leftovers.len();

            if start_point != 0 {
                data[0..start_point].copy_from_slice(&leftovers);
                leftovers.clear();
            }

            let block_size: usize = libpd_rs::functions::block_size().cast();
            let quanta = block_size * CHANNELS;

            let remaining = data.len() - start_point;
            let ticks: usize = remaining / (quanta);

            let main_block_end = start_point + ticks * quanta;
            let leftover_samples = remaining - ticks * quanta;

            ctx.receive_messages_from_pd();
            ctx.process_float(ticks.cast(), &[], &mut data[start_point..main_block_end]);
            if leftover_samples != 0 {
                if dst.len() < quanta {
                    dst.extend((dst.len()..quanta).map(|_| 0f32));
                } else if dst.len() > quanta {
                    dst.drain(quanta..);
                }

                ctx.process_float(1, &[], dst.as_mut_slice());

                data[main_block_end..(main_block_end + leftover_samples)]
                    .copy_from_slice(&dst[0..leftover_samples]);

                leftovers.extend_from_slice(&dst[leftover_samples..quanta]);
            }
        };

        let stream = Box::new(
            device
                .build_output_stream(&config, callback, err_fn, None)
                .unwrap(),
        );

        stream.play().unwrap();

        Box::leak(stream); // On macos, streams are not Send. To get around this, disown the stream.

        pd.dsp_on().unwrap();

        Self {
            //stream,
            pd_patch_watcher,
            pd_patch_rx,
            pd_patch_path,
        }
    }
}
