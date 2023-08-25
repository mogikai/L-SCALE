mod restorable;
mod tests;

use std::time::Duration;

#[derive(Debug)]
pub struct Clock {
    system_time: Duration,
    simulation_factor: f64,

    snapshots: Vec<Duration>,
}

impl Clock {
    pub fn new(current_time: Duration) -> Self {
        Self {
            system_time: current_time,
            simulation_factor: 7f64,
            snapshots: vec![],
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
        self.system_time += Duration::from_nanos((proportion as f64 * self.simulation_factor) as u64);
    }

    /// Adjust the simulation factor to the given value.
    /// The simulation factor is the amount of (virtual) time
    /// in nanoseconds taken for each instruction.
    pub fn set_simulation_factor(
        &mut self,
        simulation_factor: f64,
    ) {
        self.simulation_factor = simulation_factor;
    }
}
