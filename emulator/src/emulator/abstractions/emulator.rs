use mogi_system::errors::SchedulerError;
use std::time::Duration;

use crate::emulator::abstractions::restore::Restorable;

pub trait Scheduler: Restorable {
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError>;
}
