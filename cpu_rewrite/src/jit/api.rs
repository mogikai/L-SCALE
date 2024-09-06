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
    type Context: JitContext;
    type Value: JitValue;

    /// Creates a new context that is used during the jit runtime.
    /// The context is opaque, but implements the [JitContext] trait
    /// through which the internals may be accessed or modified.
    /// The context is owned by the [CPU] and passed to the jit runtime
    /// via the context pointer, which can then access its internals.
    fn new_context(&self) -> Self::Context;

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

    /// Emit a load instruction that loads a value from the guest address `[address + offset]` into a new variable.
    fn emit_load(
        &mut self,
        ty: (),
        address: Self::Value,
        offset: <Self::Value as JitValue>::Constant,
    ) -> Result<Self::Value, JitError>;

    /// Stores the provided value at guest address `[address + offset]`.
    fn emit_store(
        &mut self,
        address: Self::Value,
        offset: <Self::Value as JitValue>::Constant,
        value: Self::Value,
    ) -> Result<(), JitError>;

    fn emit_ret(&mut self) -> Result<(), JitError>;

    /// Push the supplied Var onto the jit stack.
    /// Note: The jit stack is separate from the
    /// targets stack. The push operations of the
    /// target are mapped to inc/dec operations on the
    /// stack pointer variable, and a subsequent load
    /// or store of the data located at the address in
    /// the variable.
    fn emit_push(
        &mut self,
        var: Self::Value,
    ) -> Result<(), JitError>;

    /// See [emit_push](Self::emit_push)
    fn emit_pop(
        &mut self,
        var: Self::Value,
    ) -> Result<(), JitError>;

    /// Emit a jmp instruction that jumps to the jit epilogue, effectively exiting the jited code
    /// and resuming execution in the emulation loop. If the block that is executed after this block
    /// is known, it can be chained directly with [chain_blocks](Self::chain_blocks) in order to avoid exiting the jit.
    fn emit_jmp_epilogue(&mut self) -> Result<(), JitError>;

    /// Chain the blocks `origin` and `target`, such that the `origin` block no longer jumps out to
    /// the epilogue block, but directly into the next block `target`.
    fn chain_blocks(
        &mut self,
        _origin_block_address: EVAddr,
        _target_block_address: EVAddr,
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

/// The jit trait deals with variables instead of registers.
/// The variables are mapped to specific registers behind the scenes.
pub trait JitValue {
    type Constant;
    type Variable;
}

/// The jit context trait defines methods that every jit context implementation must provide.
/// This facilitates access to the underlying cpu information like registers, exceptions and the PC.
pub trait JitContext {
    /// Get the current program counter. Since the register is architecture specific, this method
    /// exists for convenience so we do not have to consider all the different cases.
    fn pc(&self) -> EVAddr;

    /// TODO: RegisterAccessor type?
    fn register(
        &self,
        register: (),
    );

    fn register_mut(
        &mut self,
        register: (),
    );
}

pub type PreludeSignature = extern "C" fn(context: *mut c_void, block: JitCode);
