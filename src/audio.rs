use cpal::{
    BufferSize, SampleFormat,
    traits::StreamTrait,
    traits::{DeviceTrait, HostTrait},
};
use easy_cast::Cast;
use libpd_rs::Pd;
use log::{info, warn};
use notify::Watcher;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek},
    path::Path,
    sync::mpsc::{Receiver, Sender, channel},
    time::Duration,
};

use magnum::container::ogg::OpusSourceOgg;
use seq_macro::seq;

use crate::game::GameState;

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

#[allow(unused)]
struct PdFile {
    watcher: notify::RecommendedWatcher,
    rx: Receiver<Result<notify::Event, notify::Error>>,
    path: &'static Path,
}

#[allow(unused)]
pub struct Audio {
    pd_file: Option<PdFile>,

    pub audio_parameters_tx: Sender<AudioParameters>,
}

pub const SAMPLE_RATE: u32 = 48000;
pub const CHANNELS: usize = 2;
const BUFFER_SIZE_SAMPLES: u32 = 1024;

#[derive(Debug, Copy, Clone)]
#[allow(unused)]
pub struct AudioParameters {
    pub distance: Option<f32>,
    pub scene: GameState,
    pub time_since_last_scene_change: f32,
    pub volume: f32,
    pub sfx: Option<usize>
}

impl Default for AudioParameters {
    fn default() -> Self {
        Self {
            distance: None,
            scene: GameState::default(),
            time_since_last_scene_change: 0.0,
            volume: 0.0,
            sfx: None
        }
    }
}

macro_rules! load_prefixed_files {
    ($prefix:expr, $suffix:expr, $n:expr) => {{
        seq!(I in 0..$n {
            [
                #( include_bytes!(concat!($prefix, stringify!(I), $suffix)).as_slice(), )*
            ]
        })
    }};
}

struct AudioData {
    trombone_sounds: Vec<Vec<f32>>,
    music: Vec<Vec<[f32; 2]>>,
    sfx: Vec<Vec<[f32; 2]>>,
}

struct PlaybackInfo {
    volume: f32,
    playhead: usize,
}

#[derive(Default)]
struct AudioScratchpad {
    t: usize,
    last_sample: Option<(usize, usize, f64, usize)>,
    current_sample: Option<(usize, usize)>,
    playing_music: HashMap<usize, PlaybackInfo>,
    current_playing_track: bool,
    current_sfx: Option<usize>,
    sfx_playhead: usize,
}

const ENABLE_PD: bool = false;
const EXTERNAL_PD_PATCH: bool = false;

impl Audio {
    fn process_audio(
        audio_data: &AudioData,
        scratchpad: &mut AudioScratchpad,
        parameters: AudioParameters,
        data: &mut [f32],
    ) {
        let audio = &audio_data.trombone_sounds;

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

        let current_music = if parameters.scene == GameState::Playing {
            if scratchpad.current_playing_track {
                2
            } else {
                1
            }
        } else {
            0
        };

        if let Some(sfx) = parameters.sfx {
            scratchpad.current_sfx = Some(sfx);
            scratchpad.sfx_playhead = 0;
        }

        let music_fade_time = 1.0;

        for (i, sample) in data.chunks_mut(2).enumerate() {
            let sfx_sample = if let Some(sfx) = scratchpad.current_sfx {
                if scratchpad.sfx_playhead >= audio_data.sfx[sfx].len() {
                    scratchpad.current_sfx = None;
                    scratchpad.sfx_playhead = 0;
                    [0.0, 0.0]
                } else {
                    let sample = audio_data.sfx[sfx][scratchpad.sfx_playhead];
                    scratchpad.sfx_playhead += 1;

                    sample
                }
            } else {
                [0.0, 0.0]
            };

            if !(parameters.scene == GameState::Splash
                && parameters.time_since_last_scene_change <= 1.0)
            {
                scratchpad
                    .playing_music
                    .entry(current_music)
                    .or_insert_with(|| PlaybackInfo {
                        volume: 0.0,
                        playhead: 0,
                    });
            }

            let mut remove_list = Vec::new();
            let music_sample =
                scratchpad
                    .playing_music
                    .iter_mut()
                    .fold((0.0, 0.0), |sample, (&track, info)| {
                        if info.playhead >= audio_data.music[track].len() {
                            info.playhead = 0; // loop the audio

                            if track == 1 || track == 2 {
                                scratchpad.current_playing_track =
                                    !scratchpad.current_playing_track;
                            }
                        }

                        let out = (
                            sample.0 + info.volume * audio_data.music[track][info.playhead][0],
                            sample.1 + info.volume * audio_data.music[track][info.playhead][1],
                        );

                        info.playhead += 1;

                        // Fade in/out tracks
                        if current_music == track {
                            info.volume = (info.volume
                                + 1.0 / (music_fade_time * (SAMPLE_RATE as f32)))
                                .min(1.0)
                        } else {
                            info.volume = (info.volume
                                - 1.0 / (music_fade_time * (SAMPLE_RATE as f32)))
                                .max(0.0);

                            if info.volume == 0.0 {
                                remove_list.push(track);
                            }
                        }

                        out
                    });

            for i in remove_list {
                scratchpad.playing_music.remove(&i).unwrap();
            }

            let mut detector_volume = 0.0;
            let mut mono_sample = 0.0;
            let mut fading_sample = 0.0;

            if let Some(distance) = parameters.distance
                && (parameters.scene == GameState::Playing
                    || parameters.time_since_last_scene_change < fade_time)
            {
                let n = (19.99 / (1.0 + 0.01 * distance.powi(2))) as usize;
                let current_sample = scratchpad.current_sample.get_or_insert((n, 0));

                let (tempo_current, _num_samples_current) =
                    get_tempo_and_num_samples(current_sample.0);
                let (_tempo_new, num_samples_new) = get_tempo_and_num_samples(n);
                let tempo = tempo_current as f64 / 60.0;

                let time: f64 = (scratchpad.t as f64) / (SAMPLE_RATE as f64);

                let time_since_last_scene_change =
                    parameters.time_since_last_scene_change + (i as f32) / (SAMPLE_RATE as f32);
                detector_volume = if parameters.scene == GameState::Playing {
                    (time_since_last_scene_change / fade_time).min(1.0)
                } else {
                    (1.0 - time_since_last_scene_change / fade_time).max(0.0)
                } as f32;

                let get_sample = |t: usize, sample: usize, subsample: usize, tempo: f64| {
                    audio[sample][(t
                        + ((((SAMPLE_RATE as f64) / tempo) * (subsample as f64)) as usize))
                        .min(audio[sample].len() - 1)]
                };
                mono_sample = get_sample(scratchpad.t, current_sample.0, current_sample.1, tempo);

                fading_sample = if let Some(last_sample) = &mut scratchpad.last_sample {
                    let ret = (1.0 - 50.0 * ((scratchpad.t as f32) / (SAMPLE_RATE as f32)))
                        .max(0.0)
                        * get_sample(last_sample.3, last_sample.0, last_sample.1, last_sample.2);

                    last_sample.3 += 1;

                    ret
                } else {
                    0.0
                };

                if time * tempo >= 1.0 {
                    if n != current_sample.0 {
                        scratchpad.last_sample =
                            Some((current_sample.0, current_sample.1, tempo, scratchpad.t));
                        current_sample.0 = n;
                        current_sample.1 = 0;
                    } else {
                        current_sample.1 = (current_sample.1 + 1) % num_samples_new;
                        scratchpad.last_sample = None;
                    }
                    scratchpad.t = 0;
                }

                scratchpad.t += 1;
            }

            sample[0] = parameters.volume
                * (0.16 * detector_volume * (mono_sample + fading_sample) + 0.36 * music_sample.0) + 0.48 * sfx_sample[0];
            sample[1] = parameters.volume
                * (0.16 * detector_volume * (mono_sample + fading_sample) + 0.36 * music_sample.1) + 0.48 * sfx_sample[1];
        }
    }

    pub fn process_events(&mut self, pd: &mut Pd) {
        if let Some(pd_file) = &self.pd_file {
            let mut reload = false;
            while let Ok(event) = pd_file.rx.try_recv() {
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
            std::fs::File::open(pd_file.path)
                .unwrap()
                .read_to_string(&mut patch)
                .unwrap();

            if !patch.is_empty() {
                info!("hot reloaded {}", (pd_file.path).to_str().unwrap());
                if pd.eval_patch(&patch).is_err() {
                    warn!("pd patch error: {patch}");
                }
            }
        }
    }

    pub fn decompress_opus<const N: usize, T: Read + Seek>(file: T) -> Vec<[f32; N]> {
        let opus_file = OpusSourceOgg::new(file).unwrap();

        let resample_params = SincInterpolationParameters {
            sinc_len: 48,
            f_cutoff: 0.90,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 64,
            window: WindowFunction::Hann,
        };

        let sample_rate = opus_file.metadata.sample_rate as f64;
        let channel_count = opus_file.metadata.channel_count;
        let mut decompressed = [const { Vec::new() }; N];
        for (i, sample) in opus_file.enumerate() {
            let channel = i % N;

            decompressed[channel].push(sample);
        }

        assert_eq!(channel_count, N as u8);

        let mut resampler = SincFixedIn::<f32>::new(
            (SAMPLE_RATE as f64) / sample_rate,
            2.0,
            resample_params,
            decompressed[0].len(),
            N,
        )
        .unwrap();

        let result = resampler.process(&decompressed, None).unwrap();

        (0..result[0].len())
            .map(|i| std::array::from_fn(|j| result[j][i]))
            .collect()
    }

    pub fn new(pd: &mut Pd) -> Self {
        let ((trombone_sounds, music), sfx) = rayon::join(||
            rayon::join(
                || {
                    load_prefixed_files!("../assets/music/trombone/", ".opus", 20)
                        .par_iter()
                        .map(|file| {
                            Self::decompress_opus::<1, Cursor<&[u8]>>(Cursor::new(file))
                                .into_iter()
                                .map(|[x]| x)
                                .collect()
                        })
                        .collect()
                },
                || {
                    [
                        include_bytes!("../assets/music/solesearching.opus").as_slice(),
                        include_bytes!("../assets/music/smp_searching.opus").as_slice(),
                        include_bytes!("../assets/music/smpdanger.opus").as_slice(),
                    ]
                    .par_iter()
                    .map(|file| Self::decompress_opus::<2, _>(Cursor::new(file)))
                    .collect()
                },
            ),
            || {
                [
                    include_bytes!("../assets/sfx/shoe.opus").as_slice(),
                    include_bytes!("../assets/sfx/ring.opus").as_slice(),
                    include_bytes!("../assets/sfx/beer-bottle-lids.opus").as_slice(),
                    include_bytes!("../assets/sfx/soda-can.opus").as_slice(),
                    include_bytes!("../assets/sfx/coins-dropping.opus").as_slice(),
                ]
                .par_iter()
                    .map(|file| Self::decompress_opus::<2, _>(Cursor::new(file)))
                    .collect()
            },
        );

        let audio_data = AudioData {
            trombone_sounds,
            music,
            sfx
        };

        let (tx, pd_patch_rx) = channel();

        let pd_file = EXTERNAL_PD_PATCH.then(|| {
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

            PdFile {
                watcher: pd_patch_watcher,
                rx: pd_patch_rx,
                path: pd_patch_path,
            }
        });

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

        let patch = if let Some(pd_file) = &pd_file {
            let mut patch = String::new();
            std::fs::File::open(pd_file.path)
                .unwrap()
                .read_to_string(&mut patch)
                .unwrap();

            patch
        } else {
            String::from_utf8(include_bytes!("pd/patch.pd").into()).unwrap()
        };

        if pd.eval_patch(&patch).is_err() {
            warn!("pd patch error: {patch}");
        }

        pd.on_print(|s| println!("{s}")).unwrap();

        let (audio_parameters_tx, audio_parameters_rx) = channel::<AudioParameters>();

        let mut leftovers: Vec<f32> = vec![];
        let mut dst: Vec<f32> = vec![];
        let mut audio_parameters = AudioParameters::default();
        let mut scratchpad = AudioScratchpad::default();

        let callback = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            if ENABLE_PD {
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
            }

            let mut candidate_send_sfx = None;
            while let Ok(parameters) = audio_parameters_rx.try_recv() {
                if let Some(distance) = parameters.distance {
                    audio_parameters.distance = Some(distance);
                }

                audio_parameters.scene = parameters.scene;
                audio_parameters.time_since_last_scene_change =
                    parameters.time_since_last_scene_change;

                audio_parameters.volume = parameters.volume;

                if let Some(sfx) = parameters.sfx {
                    candidate_send_sfx = Some(sfx);
                }
            }

            audio_parameters.sfx = candidate_send_sfx;

            Audio::process_audio(&audio_data, &mut scratchpad, audio_parameters, data);
        };

        let stream = Box::new(
            device
                .build_output_stream(&config, callback, err_fn, None)
                .unwrap(),
        );

        stream.play().unwrap();

        Box::leak(stream); // On macos, streams are not Send. To get around this, disown the stream.

        if ENABLE_PD {
            pd.dsp_on().unwrap();
        }

        Self {
            pd_file,
            audio_parameters_tx,
        }
    }
}
