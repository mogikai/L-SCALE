use std::ffi::c_void;

use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_utilities::addresses::EVAddr;
use thiserror::Error;

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
    #[error("The instruction is missing an output and cannot be translated.")]
    MissingOutput,
    #[error("The instruction is missing an input parameter and cannot be translated.")]
    MissingInput,
}

pub trait MogiJit {
    type RuntimeContext: JITRuntimeContext;
    type Value: JitValue;

    /// Creates a new context that is used during the jit runtime.
    /// The context is opaque, but implements the [JITRuntimeContext] trait
    /// through which the internals may be accessed or modified.
    /// The context is owned by the [CPU] and passed to the jit runtime
    /// via the context pointer, which can then access its internals.
    fn new_runtime_context(&self) -> Self::RuntimeContext;

    /// Create a new block that starts at the provided emulated virtual address.
    /// Only one block can be built at once.
    fn new_block(
        &mut self,
        evaddr: EVAddr,
    ) -> Result<(), JitError>;

    /// Emit jited code for the provided [JIRInstruction]. The JIT compiler will allocate
    /// registers for the used varnodes and populate them, if they have not yet been populated.
    /// This might necessitate flushing registers, if no more registers are available.
    fn emit_jinst(
        &mut self,
        jinst: &JIRInstruction,
    ) -> Result<(), JitError>;

    /// Chain the blocks `origin` and `target`, such that the `origin` block no longer jumps out to
    /// the epilogue block, but directly into the next block `target`.
    fn chain_blocks(
        &mut self,
        origin_block_address: EVAddr,
        target_block_address: EVAddr,
    ) -> Result<(), JitError> {
        todo!()
    }

    /// Finalize the emitted instructions as a block and add it to the jit cache.
    /// This also resets the JIT such that a new block can be built by emitting instructions.
    fn finalize_block(&mut self) -> Result<(), JitError>;

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
pub trait JITRuntimeContext {
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
