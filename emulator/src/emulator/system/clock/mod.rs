use std::time::Duration;

#[derive(Debug)]
pub struct Clock {
    system_time: Duration,
}

impl Clock {
    pub fn new(current_time: Duration) -> Self {
        Self {
            system_time: current_time,
        }
    }

    pub fn time(&self) -> Duration {
        self.system_time
    }

    pub fn do_timestep(
        &mut self,
        proportion: u32,
    ) {
        // TODO: Pull out simulation factor and make it configurable.
        // The simulation factor determines the size of the timestep taken during each basic block.
        // let simulation_factor = 0.032f64;
        let simulation_factor = 0.32f64;
        self.system_time += Duration::from_nanos((proportion as f64 * simulation_factor) as u64);
    }
}
