use mogi_utilities::addresses::EVAddr;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum CPUError {
    #[error("TODO")]
    InvalidArch,
    #[error("TODO")]
    InvalidPermission,
    #[error("TODO")]
    Stopped(StopReason),
    #[error("TODO")]
    TaskExists,
    #[error("TODO")]
    Unknown,
}

#[derive(Error, Debug, Eq, PartialEq, Copy, Clone)]
#[repr(u64)]
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
