use std::ops::{Deref, DerefMut};

use sdl3::{event::Event, keyboard::Keycode};
use ultraviolet::Vec2;

pub struct EventLoop {
    pump: sdl3::EventPump,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone)]
pub struct Inputs {
    pub camera_rotation: Vec2,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub quit: bool,
    pub recreate_swapchain: bool,
}

#[derive(Debug)]
pub struct Input {
    inputs: Inputs,
    pub previous: Inputs,
}

impl Deref for Input {
    type Target = Inputs;

    fn deref(&self) -> &Self::Target {
        &self.inputs
    }
}

impl DerefMut for Input {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inputs
    }
}

impl Input {
    pub fn new(initial_state: Inputs) -> Self {
        Self {
            inputs: initial_state.clone(),
            previous: initial_state,
        }
    }

    pub fn update(&mut self) {
        self.previous = self.inputs.clone()
    }
}

impl Inputs {
    pub fn set_input(&mut self, key: sdl3::keyboard::Keycode, pressed: bool) {
        type K = Keycode;
        if cfg!(feature = "colemak") {
            match key {
                K::W => self.forward = pressed,
                K::R => self.backward = pressed,
                K::A => self.left = pressed,
                K::S => self.right = pressed,
                K::Space => self.up = pressed,
                K::C => self.down = pressed,
                K::Escape => self.quit = pressed,
                _ => (),
            }
        } else {
            match key {
                K::W => self.forward = pressed,
                K::S => self.backward = pressed,
                K::A => self.left = pressed,
                K::D => self.right = pressed,
                K::Space => self.up = pressed,
                K::C => self.down = pressed,
                K::Escape => self.quit = pressed,
                _ => (),
            }
        }
    }

    pub fn camera_rotation(mut self, rotation: Vec2) -> Self {
        self.camera_rotation = rotation;

        self
    }
}

fn process_event(event: Event, input: &mut Input) {
    match event {
        Event::Quit { timestamp: _ } => input.quit = true,
        Event::KeyDown {
            keycode: Some(key),
            repeat: false,
            ..
        } => input.set_input(key, true),
        Event::KeyUp {
            keycode: Some(key),
            repeat: false,
            ..
        } => input.set_input(key, false),
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

            input.camera_rotation = {
                let mut rotation = input.camera_rotation + Vec2::new(xrel, yrel) * SENSITIVITY;

                let vertical_look_limit = 0.99 * std::f32::consts::FRAC_PI_2;

                rotation.y = rotation.y.clamp(-vertical_look_limit, vertical_look_limit);

                rotation
            };
        }
        Event::Window {
            timestamp: _,
            window_id: _,
            win_event: sdl3::event::WindowEvent::PixelSizeChanged(_, _),
        } => {
            input.recreate_swapchain = true;
        }
        _ => (),
    }
}

impl EventLoop {
    pub fn new(pump: sdl3::EventPump) -> Self {
        Self { pump }
    }

    pub fn run<F: FnMut(&mut Input)>(&mut self, mut render: F) {
        let mut input = Input::new(Inputs::default().camera_rotation(Vec2::new(
            3.0 * std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_8,
        )));

        'quit: loop {
            render(&mut input);

            input.update();

            while let Some(event) = self.pump.poll_event() {
                process_event(event, &mut input);
            }

            if input.quit {
                break 'quit;
            }
        }
    }
}
