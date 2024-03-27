use mogi_system::components::restore::Restorable;
use mogi_system::errors::SchedulerError;
use std::time::Duration;

// TODO: Remove restorable supertrait
pub trait Scheduler: Restorable {
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError>;
}
