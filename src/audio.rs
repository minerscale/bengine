use cpal::{
    BufferSize, SampleFormat,
    traits::StreamTrait,
    traits::{DeviceTrait, HostTrait},
};
use easy_cast::Cast;
use libpd_rs::Pd;
use log::{info, warn};
use notify::Watcher;

use core::slice;
use std::{
    io::Read,
    path::Path,
    sync::mpsc::{Receiver, Sender, channel},
    time::Duration,
};

use include_bytes_aligned::include_bytes_aligned;
use seq_macro::seq;

use crate::game::GameState;

#[allow(unused)]
pub struct Audio {
    pub pd_patch_watcher: notify::RecommendedWatcher,
    pub pd_patch_rx: Receiver<Result<notify::Event, notify::Error>>,
    pub pd_patch_path: &'static Path,

    pub audio_parameters_tx: Sender<AudioParameters>,
}

pub const SAMPLE_RATE: u32 = 44100;
pub const CHANNELS: usize = 2;
const BUFFER_SIZE_SAMPLES: u32 = 1024;

#[derive(Debug, Copy, Clone)]
#[allow(unused)]
pub struct AudioParameters {
    pub distance: Option<f32>,
    pub scene: GameState,
    pub time_since_last_scene_change: f32,
    pub volume: f32,
}

impl Default for AudioParameters {
    fn default() -> Self {
        Self {
            distance: None,
            scene: GameState::default(),
            time_since_last_scene_change: 0.0,
            volume: 0.0
        }
    }
}

macro_rules! load_prefixed_files {
    ($prefix:expr, $n:expr) => {{
        seq!(I in 0..=$n {
            [
                #( include_bytes_aligned!(4, concat!($prefix, "/", stringify!(I))), )*
            ]
        })
    }};
}

const fn reinterpret_u8_slice_to_f32_slice<const N: usize>(
    slice: [&'static [u8]; N],
) -> [&'static [f32]; N] {
    let mut result: [&'static [f32]; N] = [&[]; N];

    let mut i = 0;

    while i < N {
        assert!(slice[i].len() % 4 == 0);

        result[i] =
            unsafe { slice::from_raw_parts(slice[i].as_ptr().cast::<f32>(), slice[i].len() / 4) };

        i += 1;
    }

    result
}

impl Audio {
    fn process_audio(
        t: &mut usize,
        last_sample: &mut Option<(usize, usize, f64, usize)>,
        current_sample: &mut Option<(usize, usize)>,
        parameters: AudioParameters,
        data: &mut [f32],
    ) {
        const DATA: [&'static [u8]; 20] =
            load_prefixed_files!("../assets/molly-trombone/", 19);

        const AUDIO: [&'static [f32]; 20] = reinterpret_u8_slice_to_f32_slice(DATA);

        fn get_tempo_and_num_samples(n: usize) -> (usize, usize) {
            const TEMPOS: [u8; 5] = [60, 72, 84, 96, 108];
            const NUM_SAMPLES: [u8; 5] = [4, 8, 8, 16, 32];

            let n = n + ((n > 16) as usize);

            let num_samples = NUM_SAMPLES[n / TEMPOS.len()].cast();
            let tempo =
                usize::from(TEMPOS[n % TEMPOS.len()]) * (2usize).pow((n / TEMPOS.len()).cast()) / 2;

            (tempo, num_samples)
        }

        let fade_time = 0.1;
        if let Some(distance) = parameters.distance && (parameters.scene == GameState::Playing || parameters.time_since_last_scene_change < fade_time) {
            let n = (19.99 / (1.0 + 0.01 * distance.powi(2))) as usize;
            let current_sample = current_sample.get_or_insert((n, 0));

            let (tempo_current, _num_samples_current) = get_tempo_and_num_samples(current_sample.0);
            let (_tempo_new, num_samples_new) = get_tempo_and_num_samples(n);
            let tempo = tempo_current as f64 / 60.0;

            for (i, sample) in data.chunks_mut(2).enumerate() {
                let time: f64 = (*t as f64) / (SAMPLE_RATE as f64);

                let time_since_last_scene_change =
                    parameters.time_since_last_scene_change + (i as f32) / (SAMPLE_RATE as f32);
                let volume = if parameters.scene == GameState::Playing {
                    (time_since_last_scene_change / fade_time).min(1.0)
                } else {
                    (1.0 - time_since_last_scene_change / fade_time).max(0.0)
                } as f32;

                let get_sample = |t: usize, sample: usize, subsample: usize, tempo: f64| {
                    AUDIO[sample][(t
                        + ((((SAMPLE_RATE as f64) / tempo) * (subsample as f64)) as usize))
                        .min(AUDIO[sample].len() - 1)]
                };
                let mono_sample = get_sample(*t, current_sample.0, current_sample.1, tempo);

                let fading_sample = if let Some(last_sample) = last_sample {
                    let ret = (1.0 - 50.0 * ((*t as f32) / (SAMPLE_RATE as f32))).max(0.0)
                        * get_sample(last_sample.3, last_sample.0, last_sample.1, last_sample.2);

                    last_sample.3 += 1;

                    ret
                } else {
                    0.0
                };

                sample[0] = parameters.volume * volume * (mono_sample + fading_sample);
                sample[1] = parameters.volume * volume * (mono_sample + fading_sample);

                if time * tempo >= 1.0 {
                    if n != current_sample.0 {
                        *last_sample = Some((current_sample.0, current_sample.1, tempo, *t));
                        current_sample.0 = n;
                        current_sample.1 = 0;
                    } else {
                        current_sample.1 = (current_sample.1 + 1) % num_samples_new;
                        *last_sample = None;
                    }
                    *t = 0;
                }

                *t += 1;
            }
        }
    }

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

        const CPAL_SAMPLE_RATE: cpal::SampleRate = cpal::SampleRate(SAMPLE_RATE);

        let supported_config = device
            .supported_output_configs()
            .expect("error while querying configs")
            .find(|c| {
                matches!(c.sample_format(), SampleFormat::F32)
                    && c.channels() == cpal::ChannelCount::try_from(CHANNELS).unwrap()
                    && (c.min_sample_rate()..=c.max_sample_rate()).contains(&CPAL_SAMPLE_RATE)
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
            .with_sample_rate(CPAL_SAMPLE_RATE);

        info!(
            "Audio Information | host: {} | device: {}",
            host.id().name(),
            device.name().unwrap()
        );

        let mut config = supported_config.config();
        config.buffer_size = BufferSize::Fixed(BUFFER_SIZE_SAMPLES);

        assert_eq!(supported_config.sample_format(), SampleFormat::F32);
        assert_eq!(supported_config.sample_rate(), CPAL_SAMPLE_RATE);

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

        let (audio_parameters_tx, audio_parameters_rx) = channel::<AudioParameters>();

        let mut leftovers: Vec<f32> = vec![];
        let mut dst: Vec<f32> = vec![];
        let mut audio_parameters = AudioParameters::default();
        let mut t = 0usize;
        let mut last_sample = None;
        let mut current_sample = None;

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

            while let Ok(parameters) = audio_parameters_rx.try_recv() {
                if let Some(distance) = parameters.distance {
                    audio_parameters.distance = Some(distance);
                }

                audio_parameters.scene = parameters.scene;
                audio_parameters.time_since_last_scene_change =
                    parameters.time_since_last_scene_change;

                audio_parameters.volume = parameters.volume;
            }

            Audio::process_audio(
                &mut t,
                &mut last_sample,
                &mut current_sample,
                audio_parameters,
                data,
            );
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
            pd_patch_watcher,
            pd_patch_rx,
            pd_patch_path,
            audio_parameters_tx,
        }
    }
}
