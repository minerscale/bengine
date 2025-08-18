use std::{
    ops::{Deref, DerefMut},
    time::{Duration, Instant},
};

use bitfield_struct::bitfield;
use easy_cast::Cast;
use log_once::warn_once;
use sdl3::event::Event;
use tracing_mutex::stdsync::Mutex;
use ultraviolet::Vec2;

use crate::{
    audio::PdEventFn,
    clock::FIXED_UPDATE_INTERVAL,
    game::GameState,
    gui::egui_sdl3_event::{sdl3_to_egui_event, sdl3_to_egui_modifiers},
};

pub struct EventLoop {
    sdl_context: sdl3::Sdl,
    window: sdl3::video::Window,
    pump: sdl3::EventPump,
}

#[bitfield(u8)]
pub struct InputBitfield {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub quit: bool,
    pub action: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone)]
pub struct Input {
    pub camera_rotation: Vec2,
    input_bitfield: InputBitfield,
}

impl Deref for Input {
    type Target = InputBitfield;

    fn deref(&self) -> &Self::Target {
        &self.input_bitfield
    }
}

impl DerefMut for Input {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.input_bitfield
    }
}

pub struct SharedState {
    inputs: Input,
    pub previous: Input,

    pub framebuffer_resized: Option<(u32, u32)>,
    game_state: GameState,
    previous_game_state: GameState,
    game_state_just_changed: bool,
    pub gui_scale: f32,
    pub last_mouse_position: Option<(f32, f32)>,
    pub audio_events: Vec<Box<PdEventFn>>,
}

impl Deref for SharedState {
    type Target = Input;

    fn deref(&self) -> &Self::Target {
        &self.inputs
    }
}

impl DerefMut for SharedState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inputs
    }
}

impl SharedState {
    pub fn new(initial_state: Input, gui_scale: f32) -> Self {
        Self {
            inputs: initial_state.clone(),
            previous: initial_state,
            framebuffer_resized: None,
            gui_scale,
            game_state: GameState::Menu,
            previous_game_state: GameState::Menu,
            game_state_just_changed: false,
            last_mouse_position: None,
            audio_events: Vec::new(),
        }
    }

    pub fn game_state(&self) -> GameState {
        self.game_state
    }

    pub fn set_game_state(&mut self, new_state: GameState) {
        self.previous_game_state = self.game_state;
        self.game_state = new_state;
        self.game_state_just_changed = true;

        self.audio_events.push(Box::new(move |pd| {
            if pd.send_symbol_to("scene", <&str>::from(new_state)).is_err() {
                warn_once!("pd: no reciever named 'scene'");
            }
        }));
    }

    pub fn update(&mut self, sdl_context: &sdl3::Sdl, window: &sdl3::video::Window) {
        self.previous = self.inputs.clone();

        if self.game_state_just_changed {
            self.game_state_just_changed = false;

            match self.game_state {
                GameState::Menu => {
                    if let Some((x, y)) = self.last_mouse_position.take() {
                        sdl_context.mouse().warp_mouse_in_window(window, x, y);
                    }
                    sdl_context.mouse().set_relative_mouse_mode(window, false);
                }
                GameState::Playing => {
                    sdl_context.mouse().set_relative_mouse_mode(window, true);
                }
            }
        }
    }
}

impl Input {
    pub fn set_input(&mut self, key: sdl3::keyboard::Scancode, pressed: bool) {
        type K = sdl3::keyboard::Scancode;
        if cfg!(feature = "colemak") {
            match key {
                K::W => self.set_forward(pressed),
                K::R => self.set_backward(pressed),
                K::A => self.set_left(pressed),
                K::S => self.set_right(pressed),
                K::Space => self.set_up(pressed),
                K::C => self.set_down(pressed),
                K::Escape => self.set_quit(pressed),
                K::F => self.set_action(pressed),
                _ => (),
            }
        } else {
            match key {
                K::W => self.set_forward(pressed),
                K::S => self.set_backward(pressed),
                K::A => self.set_left(pressed),
                K::D => self.set_right(pressed),
                K::Space => self.set_up(pressed),
                K::C => self.set_down(pressed),
                K::Escape => self.set_quit(pressed),
                K::E => self.set_action(pressed),
                _ => (),
            }
        }
    }

    pub fn camera_rotation(mut self, rotation: Vec2) -> Self {
        self.camera_rotation = rotation;

        self
    }
}

fn process_event(
    event: Event,
    shared_state: &mut SharedState,
    modifiers: egui::Modifiers,
) -> [Option<egui::Event>; 2] {
    match event {
        Event::KeyDown {
            scancode: Some(key),
            repeat: false,
            ..
        } => shared_state.set_input(key, true),
        Event::KeyUp {
            scancode: Some(key),
            repeat: false,
            ..
        } => shared_state.set_input(key, false),
        Event::MouseMotion {
            timestamp: _,
            window_id: _,
            which: _,
            mousestate: _,
            x: _,
            y: _,
            xrel,
            yrel,
        } => {
            const SENSITIVITY: f32 = 0.005;

            if shared_state.game_state() == GameState::Playing {
                shared_state.camera_rotation = {
                    let mut rotation =
                        shared_state.camera_rotation + Vec2::new(xrel, yrel) * SENSITIVITY;

                    let vertical_look_limit = 0.99 * std::f32::consts::FRAC_PI_2;

                    rotation.y = rotation.y.clamp(-vertical_look_limit, vertical_look_limit);

                    rotation
                };
            }
        }
        Event::Quit { timestamp: _ } => shared_state.set_quit(true),
        Event::Window {
            timestamp: _,
            window_id: _,
            win_event: sdl3::event::WindowEvent::PixelSizeChanged(x, y),
        } => {
            shared_state.framebuffer_resized = Some((x.cast(), y.cast()));
        }

        _ => (),
    }

    sdl3_to_egui_event(event, modifiers, shared_state.gui_scale)
}

impl EventLoop {
    pub fn new(sdl_context: sdl3::Sdl, window: sdl3::video::Window) -> Self {
        let pump = sdl_context.event_pump().unwrap();
        Self {
            sdl_context,
            window,
            pump,
        }
    }

    pub fn run<
        F: FnMut(&Mutex<SharedState>) + Send,
        G: FnMut(&Mutex<SharedState>, Vec<egui::Event>, egui::Modifiers),
    >(
        &mut self,
        mut render: F,
        mut update: G,
    ) {
        let shared_state = Mutex::new(SharedState::new(
            Input::default().camera_rotation(Vec2::new(
                3.0 * std::f32::consts::FRAC_PI_4,
                std::f32::consts::FRAC_PI_8,
            )),
            1.5,
        ));

        std::thread::scope(|scope| {
            let (quit_tx, quit_rx) = std::sync::mpsc::channel::<()>();

            let update_thread = scope.spawn(|| {
                let quit_rx = quit_rx;

                'quit: loop {
                    render(&shared_state);

                    let quit = shared_state.lock().unwrap().quit();
                    if quit {
                        quit_rx.recv().unwrap();
                        break 'quit;
                    }
                }
            });

            let mut target_time = Instant::now();
            let fixed_update_interval = Duration::from_secs_f64(FIXED_UPDATE_INTERVAL);
            'quit: loop {
                let mut state = shared_state.lock().unwrap();
                state.update(&self.sdl_context, &self.window);

                let modifiers = sdl3_to_egui_modifiers(self.sdl_context.keyboard().mod_state());
                let mut egui_events = Vec::new();
                while let Some(event) = self.pump.poll_event() {
                    for event in process_event(event, &mut state, modifiers)
                        .into_iter()
                        .flatten()
                    {
                        egui_events.push(event);
                    }
                }
                let quit = state.quit();
                drop(state);

                update(&shared_state, egui_events, modifiers);

                if quit {
                    quit_tx.send(()).unwrap();
                    update_thread.join().unwrap();
                    break 'quit;
                }

                target_time += fixed_update_interval;
                let sleep_time = target_time.duration_since(Instant::now());
                if sleep_time > Duration::ZERO {
                    std::thread::sleep(sleep_time);
                }
            }
        });
    }
}
