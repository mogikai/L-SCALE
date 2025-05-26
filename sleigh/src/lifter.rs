use self::context_provider::ContextProvider;
use crate::lifter::source::TargetInstructionSource;
use crate::spec::CustomOpId;
use crate::spec::jir::basic_block::TargetBasicBlock;
use bon::Builder;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use std::cell::Ref;
use std::num::NonZeroUsize;
use std::ptr::NonNull;
use std::time::Duration;
use thiserror::Error;

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

#[derive(Builder)]
pub struct LifterConfig<'a, D: BlockEndingOpDecider> {
    custom_op_decider: &'a D,
    pub max_instructions: Option<NonZeroUsize>,
    pub stop_addr: Option<EVAddr>,
}

impl<T: BlockEndingOpDecider> BlockEndingOpDecider for LifterConfig<'_, T> {
    fn is_block_ending_custom_op(
        &self,
        custom_op_id: CustomOpId,
    ) -> bool {
        self.custom_op_decider
            .is_block_ending_custom_op(custom_op_id)
    }
}

pub trait BlockEndingOpDecider {
    fn is_block_ending_custom_op(
        &self,
        custom_op_id: CustomOpId,
    ) -> bool;
}

/// The lifter trait defines the functionality of a lifter.
/// Lifting is the process of translating one or more target instructions to [JIR](crate::spec::jir).
/// [JIR](crate::spec::jir) is an intermediate representation used by the Mogi just in time compiler
/// to compile down to host assembly. Some Architectures decode instructions based on some cpu
/// context, e.g. long mode in x86. This context is provided via the [ContextProvider] trait.
pub trait Lifter {
    /// Lifts a basic block of target instructions starting at the specified address.
    ///
    /// # Arguments
    /// * `source` - The source providing target instruction bytes.
    /// * `context_provider` - Provider of CPU context needed for instruction decoding.
    /// This is needed for example for x86 long mode.
    /// * `addr` - Starting address of the basic block.
    /// * `max_instructions` - Optional maximum number of instructions to lift.
    /// * `stop_addr` - Optional address where to stop lifting.
    ///
    /// # Returns
    /// A `LifterResult` containing the lifted basic block or an error
    fn lift_block<S, C, D>(
        &self,
        source: &S,
        context_provider: &mut C,
        config: &LifterConfig<D>,
        addr: EVAddr,
    ) -> LifterResult<TargetBasicBlock>
    where
        S: TargetInstructionSource,
        C: ContextProvider,
        D: BlockEndingOpDecider;
}
