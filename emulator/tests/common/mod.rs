mod emulate;
mod mocks;

pub use emulate::emulate;
use fuzzyemu::emulator::interface::error::{
    SchedulerError,
    StopReason,
};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    fmt,
    EnvFilter,
};

pub fn setup_logging() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("FUZE_LOG"))
        .init();
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
