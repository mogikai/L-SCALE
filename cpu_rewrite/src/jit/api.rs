use std::ffi::c_void;

use thiserror::Error;

use mogi_utilities::addresses::EVAddr;

use crate::jit::cache::{
    JitCacheError,
    JitCode,
};

#[derive(Debug, Error)]
pub enum JitError {
    #[error("Tried to emit an instruction with no block being active. Pleas call activate_block first.")]
    NoActiveBlock,
    #[error("Encountered an IO error during a jit operation.")]
    IOError,
    #[error("The JIT cache encountered an error.")]
    CacheError(#[from] JitCacheError),
    #[error("The assembler produced an error, while trying to assemble the basic block.")]
    AssemblerError,
    #[error("The block does not refer to the epilogue and can not be chained.")]
    NoEpilogue,
}

pub trait MogiJit {
    type BlockId: Copy + Clone;
    /// Create a new block that starts at the provided emulated virtual address.
    /// Before emitting instructions to the block, it needs to be activated with
    /// the [activate_block](Self::activate_block) method.
    fn new_block(
        &mut self,
        evaddr: EVAddr,
    ) -> Result<Self::BlockId, JitError>;

    /// Activate the block identified by `block_id` for emitting instructions.
    /// Instructions will only be emitted to the currently active block.
    /// The active block may be finalized with the [finalize_block](Self::finalize_block) method,
    /// removing it from the unfinished blocks and storing it in the jit
    /// cache for further access.
    fn activate_block(
        &mut self,
        block_id: Self::BlockId,
    ) -> Result<(), JitError>;

    fn emit_ret(&mut self) -> Result<(), JitError>;

    /// Emit a jmp instruction that jumps to the jit epilogue, effectively exiting the jited code
    /// and resuming execution in the emulation loop. If the block that is executed after this block
    /// is known, it can be chained directly with [chain_blocks](Self::chain_blocks) in order to avoid exiting the jit.
    fn emit_jmp_epilogue(&mut self) -> Result<(), JitError>;

    /// Chain the blocks `origin` and `target`, such that the `origin` block no longer jumps out to
    /// the epilogue block, but directly into the next block `target`.
    fn chain_blocks(
        &mut self,
        origin_block_address: EVAddr,
        target_block_address: EVAddr,
    ) -> Result<(), JitError> {
        todo!()
    }

    /// Finalize the block and add it to the jit cache.
    fn finalize_block(
        &mut self,
        block_id: Self::BlockId,
    ) -> Result<(), JitError>;

    /// Get the block starting at emulated virtual address `evaddr`.
    /// If no block is registered for this emulated virtual address,
    /// None is returned.
    fn fetch_block(
        &self,
        evaddr: EVAddr,
    ) -> Result<JitCode, JitError>;

    /// Fetch the jit prelude function. This is effectively the entrypoint
    /// of the jit. The signature of the returned jit prelude function is
    /// specified here: [PreludeSignature]
    fn prelude(&self) -> Result<PreludeSignature, JitError>;
}

pub type PreludeSignature = extern "C" fn(context: *mut c_void, block: JitCode);
