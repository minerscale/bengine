use std::time::Instant;

pub const FIXED_UPDATE_INTERVAL: f64 = 1.0 / 120.0;

#[derive(Debug, Clone)]
pub struct Clock {
    pub start_time: Instant,
    pub previous_time: Instant,
    pub time: f32,
    pub dt: f32,
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock {
    pub fn new() -> Self {
        let start_time = std::time::Instant::now();

        let previous_time = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs_f64(FIXED_UPDATE_INTERVAL))
            .unwrap();

        let dt = (start_time - previous_time).as_secs_f32();
        let time = 0.0;

        Self {
            start_time,
            previous_time,
            time,
            dt,
        }
    }

    pub fn update(&mut self) {
        let new_time = std::time::Instant::now();

        self.dt = (new_time - self.previous_time).as_secs_f32();
        self.time = (new_time - self.start_time).as_secs_f32();

        self.previous_time = new_time;
    }
}
