use std::time::Duration;

use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::interface::error::SchedulerError;

pub trait Scheduler: Restorable {
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError>;
}
