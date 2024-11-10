use sdl2::{event::Event, keyboard::Keycode};

pub struct EventLoop {
    pump: sdl2::EventPump,

    pub time: f32,
    pub framebuffer_resized: bool,
}

impl EventLoop {
    pub fn new(pump: sdl2::EventPump) -> Self {
        EventLoop {
            pump,
            time: 0.0,
            framebuffer_resized: false,
        }
    }

    pub fn run<F: FnMut(&mut Self)>(&mut self, mut f: F) {
        'quit: loop {
            f(self);

            self.time += 1.0 / 60.0;

            while let Some(event) = self.pump.poll_event() {
                match event {
                    Event::Quit { timestamp: _ } => break 'quit,
                    Event::KeyDown {
                        timestamp: _,
                        window_id: _,
                        keycode: Some(Keycode::Escape),
                        scancode: _,
                        keymod: _,
                        repeat: _,
                    } => break 'quit,
                    Event::Window {
                        timestamp: _,
                        window_id: _,
                        win_event: sdl2::event::WindowEvent::SizeChanged(_, _),
                    } => self.framebuffer_resized = true,
                    _ => (),
                }
            }
        }
    }
}
