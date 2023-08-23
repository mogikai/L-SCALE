mod emulate;

pub use emulate::emulate;
use fuzzyemu::emulator::abstractions::emulator::StopReason;

pub fn setup_logging() {
    let _ = pretty_env_logger::try_init();
}

#[derive(Debug)]
pub enum TestFailure {
    ProcessFailure(StopReason),
    Unknown,
}
