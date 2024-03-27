pub mod cpu;
pub mod filesystem;
pub mod generic;

use cpu::{
    CPUError,
    StopReason,
};
use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum EmulatorError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The emulator encountered a cpu related error")]
    CPUError(#[from] CPUError),
}
