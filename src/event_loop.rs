pub struct EventLoop {
    pump: sdl2::EventPump,
}

impl EventLoop {
    pub fn new(pump: sdl2::EventPump) -> Self {
        EventLoop { pump }
    }

    pub fn run<F: FnMut(), G: FnMut(sdl2::event::Event) -> bool>(
        &mut self,
        mut render: F,
        mut process_event: G,
    ) {
        'quit: loop {
            render();

            while let Some(event) = self.pump.poll_event() {
                if process_event(event) {
                    break 'quit;
                }
            }
        }
    }
}
