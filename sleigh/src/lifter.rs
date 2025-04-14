use crate::lifter::source::TargetInstructionSource;
use crate::spec::jir::basic_block::TargetBasicBlock;
use mogi_utilities::addresses::EVAddr;
use std::cell::Ref;
use std::num::NonZeroUsize;
use std::ptr::NonNull;
use std::time::Duration;
use thiserror::Error;

use self::context_provider::ContextProvider;

pub mod cache;
pub mod context_provider;
pub mod source;

pub type LifterResult<T = ()> = Result<T, LifterError>;

#[derive(Debug, Error, Clone)]
pub enum LifterError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The address {0:?} is already associated with a cached instruction")]
    InstructionAlreadyCached(EVAddr),
    #[error("The address {0:?} is already associated with a cached block")]
    BlockAlreadyCached(EVAddr),
    #[error("The cache has no entry for the requested address {0:?}")]
    MissingCacheEntry(EVAddr),
    #[error("Could not load input bytes from address {0:?}")]
    InputLoad(EVAddr),
    #[error("There are not enough input bytes left")]
    EndOfBytes,
    #[error("Input bytes were provided but did not match anything: {0:X?}")]
    NoMatch(Vec<u8>),
}

/// The lifter trait defines the functionality of a lifter.
/// Lifting is the process of translating one or more target instructions to [JIR](crate::spec::jir).
/// [JIR](crate::spec::jir) is an intermediate representation used by the Mogi just in time compiler
/// to compile down to host assembly. Some Architectures decode instructions based on some cpu
/// context, e.g. long mode in x86. This context is provided via the [ContextProviderr] trait.
pub trait Lifter {
    fn lift_block<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr: EVAddr,
        max_instructions: Option<NonZeroUsize>,
    ) -> LifterResult<TargetBasicBlock>;

    fn diagnostics(&self) -> Ref<LifterDiagnostics>;
    fn reset_diagnostics(&mut self);
}

#[derive(Debug, Default)]
pub struct LifterDiagnostics {
    pub(crate) total_time_match: Duration,
    pub(crate) total_time_build: Duration,
    pub(crate) inst_lifted_since_last_reset: u32,
}

impl LifterDiagnostics {
    pub fn match_avg(&self) -> Duration {
        self.total_time_match / self.inst_lifted_since_last_reset
    }

    pub fn build_avg(&self) -> Duration {
        self.total_time_build / self.inst_lifted_since_last_reset
    }
}
