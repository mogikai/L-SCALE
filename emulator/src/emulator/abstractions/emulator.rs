use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub enum StopReason {
    Unknown,
    Killed,
    MissingSyscall,
    ProcessExit,
}

pub trait Emulator {
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), StopReason>;
}
