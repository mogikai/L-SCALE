use crate::jit::cache::JitCacheError;
use crate::jit::code_buffer::CodeBufferError;
use crate::tlb::{
    AccessType,
    TLB,
};
use dynasmrt::DynasmError;
use mogi_mem::physical::page::PageData;
use mogi_sleigh::lifter::context_provider::ContextProvider;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;
use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
    VarNodeId,
};
use mogi_sleigh::spec::{
    CustomOpId,
    SpaceId,
};
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JitError {
    #[error("Tried to emit an instruction with no block being active. Pleas call activate_block first.")]
    NoActiveBlock,
    #[error("Encountered an IO error during a jit operation.")]
    IOError,
    #[error("The JIT cache encountered an error.")]
    CacheError(#[from] JitCacheError),
    #[error("The JIT cache has no entry for the requested block at address {0:?}")]
    BlockNotFound(EVAddr),
    #[error("The assembler produced an error, while trying to assemble the basic block.")]
    AssemblerError(#[from] DynasmError),
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
    #[error(
        "The tlb callback for access type {0} is missing. Please register a callback using register_tlb_read_miss_callback or register_tlb_write_miss_callback."
    )]
    MissingTlbCallback(AccessType),
    #[error("The jit encountered a buffer error.")]
    CodeBufferError(#[from] CodeBufferError),
}

#[derive(Error, Debug, Copy, Clone)]
pub enum RuntimeError {
    #[error("The jit encountered a permission error.")]
    PermissionError,
    #[error("The instruction is missing an output and cannot be translated.")]
    MissingOutput,
    #[error("The instruction is missing an input parameter and cannot be translated.")]
    MissingInput,
}

pub type JITResult<T = ()> = Result<T, JitError>;

pub trait MogiJit {
    type RuntimeContext: JITTaskRuntimeContext + ContextProvider + Sized;
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
        CB: CustomOpImpl<Self::RuntimeContext>;

    fn register_tlb_read_miss_callback<CB>(
        &mut self,
        callback: CB,
    ) -> JITResult
    where
        CB: TlbMissHelper<Self::RuntimeContext>;

    fn register_tlb_write_miss_callback<CB>(
        &mut self,
        callback: CB,
    ) -> JITResult
    where
        CB: TlbMissHelper<Self::RuntimeContext>;

    /// Compiles the provided target basic block and inserts it into the jit cache for future access.
    fn compile_target_block(
        &mut self,
        block: &TargetBasicBlock,
    ) -> JITResult;

    /// Chain the blocks `origin` and `target`, such that the `origin` block no longer jumps out to
    /// the epilogue block, but directly into the next block `target`.
    fn chain_blocks(
        &mut self,
        origin_block_address: EVAddr,
        target_block_address: EVAddr,
        path: JITPathChoice,
    ) -> JITResult;

    /// Execute the block at the specified emulated virtual address.
    /// If there is no translated block, an error is returned.
    fn execute(
        &self,
        context: &mut Self::RuntimeContext,
        block: EVAddr,
    ) -> Result<(), JitError>;

    /// Checks if the block at the specified adress is already translated and in the execution
    /// cache.
    fn is_cached(
        &self,
        block: EVAddr,
    ) -> bool;

    /// Read the architecture specific instruction pointer value from the runtime context.
    /// This looks up the correct varnode according to the specification and returns the current value.
    /// The value can also be accessed by providing the varnode id of the instruction pointer to
    /// the [varnode](Self::varnode) function.
    fn instruction_pointer(
        &self,
        context: &Self::RuntimeContext,
    ) -> Result<EVAddr, JitError>;

    /// Set the architecture specific instruction pointer value in the runtime context.
    /// This looks up the correct varnode according to the specification and sets the value to `ip`.
    /// For convenience, the old value of the instruction pointer is returned.
    fn set_instruction_pointer(
        &self,
        context: &mut Self::RuntimeContext,
        ip: EVAddr,
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
/// The context also owns the [TLB] which the mmu can access to populate entries.
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

    /// Get a reference to the tlb of this task context
    fn tlb(&self) -> &TLB;

    /// Get a mutable reference to the tlb of this task context.
    fn tlb_mut(&mut self) -> &mut TLB;

    /// Callbacks may set a runtime error in the context.
    /// This causes the jit to exit as soon as possible,
    /// returning the set error. Some callbacks may cause
    /// the jit to immediately exit, flushing any cached
    /// varnodes to context.
    fn set_runtime_error(
        &mut self,
        error: RuntimeError,
    );

    fn runtime_error(&self) -> Option<RuntimeError>;
}

/// The runtime context contains information that is global to all tasks (processes/threads).
/// This can include information like callbacks, runtime helpers, etc.
pub trait JITGlobalRuntimeContext {}

/// # Safety
/// Calling the prelude function is undefined behavior if one of the following condition is not met:
/// * `context` must be a valid pointer to a [MogiJit::RuntimeContext].
/// * `block` must point to a range of executable memory associated with a block inside a [CodeBuffer](crate::jit::code_buffer::CodeBuffer).
pub type PreludeSignature = unsafe extern "C" fn(context: *mut c_void, block: NonNull<u8>);

/// A custom operation must have a signature that corresponds to this trait.
pub trait CustomOpImpl<RuntimeContextT>: Fn(&mut RuntimeContextT, &CustomOpArgs) -> Result<(), RuntimeError>
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

/// The tlb miss helpers must have a signature that corresponds to this trait.
pub trait TlbMissHelper<RuntimeContextT>:
    Fn(&mut RuntimeContextT, EVAddr, usize, SpaceId) -> Result<HVAddr<PageData>, RuntimeError>
where
    RuntimeContextT: JITTaskRuntimeContext,
{
}

impl<T, RuntimeContextT> TlbMissHelper<RuntimeContextT> for T
where
    T: Fn(&mut RuntimeContextT, EVAddr, usize, SpaceId) -> Result<HVAddr<PageData>, RuntimeError>,
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
    pub output: Option<JIRSlice>,
    pub input_a: Option<JIRValue>,
    pub input_b: Option<JIRValue>,
    pub extra_args: Vec<JIRValue>,
}

pub enum JITPathChoice {
    Main,
    Alternate,
}
