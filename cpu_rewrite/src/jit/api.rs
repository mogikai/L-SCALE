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
    #[error("The jit encountered a runtime error.")]
    RuntimeError(#[from] RuntimeError),
}

#[derive(Error, Debug, Copy, Clone)]
pub enum RuntimeError {
    #[error("The instruction is missing an output and cannot be translated.")]
    MissingOutput,
    #[error("The instruction is missing an input parameter and cannot be translated.")]
    MissingInput,
}

pub type JITResult<T = ()> = Result<T, JitError>;

pub trait MogiJit {
    type RuntimeContext: JITTaskRuntimeContext + Sized;
    type GlobalContext: JITGlobalRuntimeContext + Sized;

    /// Creates a new context that is used during the jit runtime.
    /// The context is opaque, but implements the [JITTaskRuntimeContext] trait
    /// through which the internals may be accessed or modified.
    /// The context is owned by the [CPU] and passed to the jit runtime
    /// via the context pointer, which can then access its internals.
    fn new_runtime_context(&mut self) -> Self::RuntimeContext;

    /// Register a function as implementation of the custom operation identified by `name`
    fn register_custom_op_impl<CB>(
        &mut self,
        op_id: CustomOpId,
        callback: CB,
    ) -> JITResult
    where
        CB: Fn(&mut Self::RuntimeContext, &CustomOpArgs) -> Result<(), RuntimeError> + 'static;

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
        _origin_block_address: EVAddr,
        _target_block_address: EVAddr,
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
/// This context is task specific. I.e. each task has its own context.
pub trait JITTaskRuntimeContext {
    fn slice(
        &self,
        slice: JIRSlice,
    ) -> Result<&[u8], RuntimeError>;

    fn slice_mut(
        &mut self,
        slice: JIRSlice,
    ) -> Result<&mut [u8], RuntimeError>;
}

/// The runtime context contains information that is global to all tasks (processes/threads).
/// This can include information like callbacks, runtime helpers, etc.
pub trait JITGlobalRuntimeContext {}

pub type PreludeSignature = extern "C" fn(context: *mut c_void, block: JitCode);

/// A custom operation must have a signature that corresponds to this trait.
pub trait CustomOpImpl<RuntimeContextT>:
    FnMut(&mut RuntimeContextT, &CustomOpArgs) -> Result<(), RuntimeError>
where
    RuntimeContextT: JITTaskRuntimeContext,
{
}

impl<T, RuntimeContextT> CustomOpImpl<RuntimeContextT> for T
where
    T: Fn(&mut RuntimeContextT, &CustomOpArgs) -> Result<(), RuntimeError>,
    RuntimeContextT: JITTaskRuntimeContext,
{
}

/// The arguments passed to a custom operation. The output is always a [JIRSlice] if it exists, since
/// [JIRConst](mogi_sleigh::spec::jir::values::JIRConst) cannot be written to.
/// For ease of access, the arguments are wrapped in a Result. Accessing an argument can be done with the
/// `?` operator. If the argument is missing, then this will currently result in a panic, until
/// the jit runtime gets an updated error handling mechanism.
#[derive(Debug)]
pub struct CustomOpArgs {
    pub output: Result<JIRSlice, RuntimeError>,
    pub input_a: Result<JIRValue, RuntimeError>,
    pub input_b: Result<JIRValue, RuntimeError>,
    pub extra_args: Vec<JIRValue>,
}
