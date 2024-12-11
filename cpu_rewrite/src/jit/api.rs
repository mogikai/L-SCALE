use crate::jit::cache::{
    JitCacheError,
    JitCode,
};
use mogi_sleigh::spec::CustomOpId;
use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
    VarNodeId,
};
use mogi_utilities::addresses::EVAddr;
use num_traits::FromBytes;
use std::ffi::c_void;
use thiserror::Error;

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
    #[error("The instruction is missing an output and cannot be translated.")]
    MissingOutput,
    #[error("The instruction is missing an input parameter and cannot be translated.")]
    MissingInput,
    #[error("The register cannot be freed since it is not a temporary.")]
    InvalidTempFree,
    #[error("The register is already allocated and cannot be reused.")]
    AlreadyAllocated,
    #[error("All registers are allocated already, and the allocator was told not to spill.")]
    NoRegistersAvailable,
    #[error("It is only possible to move slices from one register to another.")]
    CannotMoveNonSlice,
    #[error("The provided slice is not loaded.")]
    SliceNotLoaded,
}

pub type JITResult<T = ()> = Result<T, JitError>;

pub trait MogiJit {
    type RuntimeContext: JITRuntimeContext + Sized;

    /// Creates a new context that is used during the jit runtime.
    /// The context is opaque, but implements the [JITRuntimeContext] trait
    /// through which the internals may be accessed or modified.
    /// The context is owned by the [CPU] and passed to the jit runtime
    /// via the context pointer, which can then access its internals.
    fn new_runtime_context(&self) -> Self::RuntimeContext;

    ///
    fn set_custom_op_callback<CB>(
        &mut self,
        callback: Box<CB>,
    ) -> JITResult
    where
        CB: Fn(CustomOpId, JIRSlice, JIRValue, JIRValue);

    /// Create a new block that starts at the provided emulated virtual address.
    /// Only one block can be built at once.
    fn new_block(
        &mut self,
        evaddr: EVAddr,
    ) -> JITResult;

    /// Emit jited code for the provided [JIRInstruction]. The JIT compiler will allocate
    /// registers for the used varnodes and populate them, if they have not yet been populated.
    /// This might necessitate flushing registers, if no more registers are available.
    fn emit_jinst(
        &mut self,
        jinst: &JIRInstruction,
    ) -> JITResult;

    /// Flush all varnodes to memory. This clears all host registers and flushes their contents
    /// to the associated varnodes.
    fn emit_flush_slices(&mut self) -> JITResult;

    /// Chain the blocks `origin` and `target`, such that the `origin` block no longer jumps out to
    /// the epilogue block, but directly into the next block `target`.
    fn chain_blocks(
        &mut self,
        origin_block_address: EVAddr,
        target_block_address: EVAddr,
    ) -> JITResult {
        todo!()
    }

    /// Finalize the emitted instructions as a block and add it to the jit cache.
    /// This also resets the JIT such that a new block can be built by emitting instructions.
    fn finalize_block(&mut self) -> JITResult;

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

    /// Read the architecture specific instruction pointer value from the runtime context.
    /// This looks up the correct varnode according to the specification and returns the current value.
    /// The value can also be accessed by providing the varnode id of the instruction pointer to
    /// the [varnode](Self::varnode) function.
    fn instruction_pointer(
        &self,
        context: &Self::RuntimeContext,
    ) -> Result<EVAddr, JitError>;

    /// Get a reference to the bytes of the underlying varnode from the runtime context.
    fn varnode<'a>(
        &self,
        context: &'a Self::RuntimeContext,
        varnode: VarNodeId,
    ) -> Result<&'a [u8], JitError>;

    /// Get a mutable reference to the bytes of the underlying varnode from the runtime context.
    fn varnode_mut<'a>(
        &self,
        context: &'a mut Self::RuntimeContext,
        varnode: VarNodeId,
    ) -> Result<&'a mut [u8], JitError>;
}

/// The jit context trait defines methods that every jit context implementation must provide.
/// This facilitates access to the underlying cpu information like registers, exceptions and the PC.
pub trait JITRuntimeContext {
    fn slice(
        &self,
        slice: JIRSlice,
    ) -> Result<&[u8], JitError>;

    fn slice_mut(
        &mut self,
        slice: JIRSlice,
    ) -> Result<&mut [u8], JitError>;
}

pub type PreludeSignature = extern "C" fn(context: *mut c_void, block: JitCode);
