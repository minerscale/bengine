use sdl2::keyboard::Keycode;
use ultraviolet::Vec2;

pub struct EventLoop {
    pump: sdl2::EventPump,
}

#[derive(Debug, Default)]
pub struct Inputs {
    pub camera_rotation: Vec2,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
}

impl Inputs {
    pub fn set_input(&mut self, key: sdl2::keyboard::Keycode, pressed: bool) {
        type K = Keycode;
        match key {
            K::W => self.forward = pressed,
            K::R => self.backward = pressed,
            K::A => self.left = pressed,
            K::S => self.right = pressed,
            K::SPACE => self.up = pressed,
            K::C => self.down = pressed,
            _ => (),
        }
    }

    pub fn camera_rotation(mut self, rotation: Vec2) -> Self {
        self.camera_rotation = rotation;

        self
    }
}

impl EventLoop {
    pub fn new(pump: sdl2::EventPump) -> Self {
        EventLoop { pump }
    }

    pub fn run<F: FnMut(&Inputs, &mut bool), G: FnMut(sdl2::event::Event, &mut Inputs, &mut bool) -> bool>(
        &mut self,
        mut render: F,
        mut process_event: G,
    ) {
        let mut inputs = Inputs::default().camera_rotation(Vec2::new(std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_8));

        let mut framebuffer_resized = false;

        'quit: loop {
            render(&inputs, &mut framebuffer_resized);

            while let Some(event) = self.pump.poll_event() {
                if process_event(event, &mut inputs, &mut framebuffer_resized) {
                    break 'quit;
                }
            }
        }
    }
}
