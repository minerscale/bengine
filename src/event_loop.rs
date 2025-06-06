use sdl3::keyboard::Keycode;
use ultraviolet::Vec2;

pub struct EventLoop {
    pump: sdl3::EventPump,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
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

impl EventLoop {
    pub fn new(pump: sdl3::EventPump) -> Self {
        Self { pump }
    }

    pub fn run<F: FnMut(&mut Inputs), G: FnMut(sdl3::event::Event, &mut Inputs)>(
        &mut self,
        mut render: F,
        mut process_event: G,
    ) {
        let mut inputs = Inputs::default().camera_rotation(Vec2::new(
            3.0 * std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_8,
        ));

        'quit: loop {
            render(&mut inputs);

            while let Some(event) = self.pump.poll_event() {
                process_event(event, &mut inputs);
            }

            if inputs.quit {
                break 'quit;
            }
        }
    }
}
