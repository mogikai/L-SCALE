//! TODO: This will be moved to the system crate once the new api is fixed.

use crate::cpu::StopReason;
use mogi_system::errors::cpu::CPUError;
use std::num::NonZeroUsize;
use zerocopy::{
    FromBytes,
    Immutable,
    IntoBytes,
    KnownLayout,
};

pub type CPUResult<T> = Result<T, CPUError>;

#[derive(Debug, Copy, Clone)]
pub enum RunLimit {
    Unlimited,
    InstructionCount(NonZeroUsize),
}

pub trait CPU {
    /// Run the cpu emulator. If `limit` is Unlimited, then the emulator will run indefinitely.
    /// (TODO: What about yielding or internally issued stops?)
    /// Otherwise, the emulator will attempt to execute at least `limit` instructions and terminate
    /// as soon as possible, once the required instructions have been executed. TODO: Execute exactly these insts.
    fn run(
        &mut self,
        limit: RunLimit,
    ) -> Result<(), StopReason>;

    fn register<T>(
        &self,
        name: &str,
    ) -> CPUResult<&T>
    where
        T: FromBytes + KnownLayout + Immutable + IntoBytes;

    fn register_mut<T>(
        &mut self,
        name: &str,
    ) -> CPUResult<&mut T>
    where
        T: FromBytes + KnownLayout + Immutable + IntoBytes;
}
