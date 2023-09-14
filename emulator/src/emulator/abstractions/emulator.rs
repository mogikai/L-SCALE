use std::ops::Try;
use std::time::Duration;

use crate::emulator::abstractions::cpu::address::EVAddr;
use thiserror::Error;

use crate::emulator::abstractions::cpu::CPUError;
use crate::emulator::abstractions::restore::Restorable;

#[derive(Error, Debug, Copy, Clone)]
pub enum StopReason {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The process was killed by a signal")]
    Killed(u64),
    #[error("The process tried to execute a syscall that is not yet implemented")]
    MissingSyscall(EVAddr),
    #[error("The process exited normally")]
    ProcessExit(u64),
    /// The yield stop reason can be issued by any part inside a currently executed syscall.
    /// The syscall that issues the yield is responsible for being sound on being called a
    /// second time. This can be achieved either by implementing a state machine inside the
    /// syscall, or by only yielding if no changes to the kernel have been made.
    #[error("The process yielded execution to the scheduler")]
    Yield(EVAddr),
    #[error("A breakpoint was reached and the execution stopped")]
    Breakpoint,
}

#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The emulation stopped because all processes are in a stopped state")]
    Stopped(#[from] StopReason),
    #[error("The emulation stopped because a cpu error occurred")]
    CPUError(#[from] CPUError),
    #[error("The emulation stopped because no task can be scheduled since all would block again")]
    NoRunnableTasks,
}

pub trait Scheduler: Restorable {
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError>;
}
