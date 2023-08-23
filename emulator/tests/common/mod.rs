mod emulate;

pub use emulate::emulate;
use fuzzyemu::emulator::abstractions::emulator::StopReason;

#[derive(Debug)]
pub enum TestFailure {
    ProcessFailure(StopReason),
    Unknown,
}
