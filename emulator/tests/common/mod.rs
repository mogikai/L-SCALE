mod emulate;
mod mocks;

pub use emulate::emulate;
use fuzzyemu::emulator::interface::error::{
    SchedulerError,
    StopReason,
};

pub fn setup_logging() {
    let _ = pretty_env_logger::try_init();
}

#[derive(Debug)]
pub enum TestFailure {
    ProcessFailure(StopReason),
    SchedulerFailure(SchedulerError),
    Unknown,
}

impl From<SchedulerError> for TestFailure {
    fn from(e: SchedulerError) -> Self {
        return Self::SchedulerFailure(e);
    }
}
