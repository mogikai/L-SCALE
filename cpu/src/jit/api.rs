use crate::jit::cache::JitCacheError;
use crate::jit::code_buffer::CodeBufferError;
use crate::tlb::{
    AccessType,
    TLB,
};
use dynasmrt::DynasmError;
use mogi_mem::physical::page::PageData;
use mogi_sleigh::lifter::BlockEndingOpDecider;
use mogi_sleigh::lifter::context_provider::ContextProvider;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;
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
use mogi_utilities::index_map::MapKey;
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
    #[error("There was no error")]
    None,
    #[error("The jit encountered a permission error.")]
    PermissionError,
    #[error("The instruction is missing an output and cannot be translated.")]
    MissingOutput,
    #[error("The instruction is missing an input parameter and cannot be translated.")]
    MissingInput,
    #[error("The instruction encountered a division error")]
    DivisionError,
    #[error("A custom operation needs to be evaluated.")]
    CustomOp(CustomOpId),
    #[error("Unknown error")]
    Unknown,
}

impl From<RuntimeError> for u64 {
    fn from(value: RuntimeError) -> Self {
        match value {
            RuntimeError::None => 0,
            RuntimeError::PermissionError => 1,
            RuntimeError::MissingOutput => 2,
            RuntimeError::MissingInput => 3,
            RuntimeError::DivisionError => 4,
            RuntimeError::CustomOp(op_id) => {
                5u64 | ((u32::try_from(op_id.to_index()).expect("CustomOpId is a 16 bit integer") as u64) << 32)
            }
            RuntimeError::Unknown => u32::MAX as u64,
        }
    }
}

impl From<u64> for RuntimeError {
    fn from(value: u64) -> Self {
        match value as u32 {
            0 => RuntimeError::None,
            1 => RuntimeError::PermissionError,
            2 => RuntimeError::MissingOutput,
            3 => RuntimeError::MissingInput,
            4 => RuntimeError::DivisionError,
            5 => RuntimeError::CustomOp(
                CustomOpId::try_from_index((value >> 32) as usize)
                    .expect("The encoding process must have made sure that the index fits in a u16"),
            ),
            u32::MAX => RuntimeError::Unknown,
            _ => panic!("Invalid decoding of runtime error. Possibly missing case?"),
        }
    }
}

pub type JITResult<T = ()> = Result<T, JitError>;

pub trait MogiJit: BlockEndingOpDecider {
    type RuntimeContext: JITTaskRuntimeContext + ContextProvider + Sized;
    type GlobalContext: JITGlobalRuntimeContext + Sized;

    /// Creates a new context that is used during the jit runtime.
    /// The context is opaque, but implements the [JITTaskRuntimeContext] trait
    /// through which the internals may be accessed or modified.
    /// The context is owned by the [CPU] and passed to the jit runtime
    /// via the context pointer, which can then access its internals.
    fn new_runtime_context(&mut self) -> Self::RuntimeContext;

    /// A custom operation can also be implemented via the fast impl mechanism.
    /// The jit will load all the necessary slices into the arguments and write the output
    /// back into the slice specified in the instruction.
    fn register_binary_custom_op_fast_impl(
        &mut self,
        op_id: CustomOpId,
        fn_impl: CustomOpFastImplBinary,
    ) -> JITResult;

    fn register_unary_custom_op_fast_impl(
        &mut self,
        op_id: CustomOpId,
        fn_impl: CustomOpFastImplUnary,
    ) -> JITResult;

    /// For fast impls that operate on larger slices this function allows implementing
    /// operations with one output and up to two inputs that directly use the byte slices.
    fn register_custom_op_fast_impl_slice(
        &mut self,
        op_id: CustomOpId,
        fn_impl: extern "sysv64" fn(&mut [u8], &[u8], &[u8]),
    ) -> JITResult;

    /// Register a function as implementation of the custom operation identified by `name`
    fn register_custom_op_impl<CB>(
        &mut self,
        op_id: CustomOpId,
        callback: CB,
    ) -> JITResult
    where
        CB: CustomOpImpl<Self::RuntimeContext>;

    /// This function marks the provided custom op as externally implemented.
    /// This will force the jit to exit, raising an exception.
    /// The operation can then be performed, changing the cpu context as needed, after
    /// which the jit can be entered again. Any context is completely flushed before exiting,
    /// just like when normally exiting the jit.
    /// Since this incurs quite a lot of overhead, this should only be used for very expensive
    /// custom ops, like syscalls, that have to perform significant calculations anyway.
    /// One can think of this as a kind of interrupt mechanism.
    ///
    /// CAVEAT: These custom ops may only appear at the end of the semantic section of an instruction.
    ///         If any operations follow such a custom op, this is an error and will result in the jit
    ///         returning an error.
    fn mark_custom_op_as_externally_implemented(
        &mut self,
        op_id: CustomOpId,
    ) -> JITResult;

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

    /// Flush the translation buffer.
    fn flush_code(&mut self);

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

// TODO: Maybe we could also use the vectorcall abi for large inputs if we perform simd on them?
//       This depends on https://github.com/rust-lang/rust/issues/124485
#[derive(Debug, Copy, Clone)]
pub struct CustomOpFastImplBinary {
    pub impl_8: Option<extern "sysv64" fn(u8, u8) -> u8>,
    pub impl_16: Option<extern "sysv64" fn(u16, u16) -> u16>,
    pub impl_32: Option<extern "sysv64" fn(u32, u32) -> u32>,
    pub impl_64: Option<extern "sysv64" fn(u64, u64) -> u64>,
    pub impl_128: Option<extern "sysv64" fn(u128, u128) -> u128>,
    pub impl_slice_non_overlapping: unsafe extern "sysv64" fn(*mut u8, usize, *const u8, usize, *const u8, usize),
    pub impl_slice_in_out: unsafe extern "sysv64" fn(*mut u8, usize, *const u8, usize),
}

#[derive(Debug, Copy, Clone)]
pub struct CustomOpFastImplUnary {
    pub impl_8: Option<extern "sysv64" fn(u8) -> u8>,
    pub impl_16: Option<extern "sysv64" fn(u16) -> u16>,
    pub impl_32: Option<extern "sysv64" fn(u32) -> u32>,
    pub impl_64: Option<extern "sysv64" fn(u64) -> u64>,
    pub impl_128: Option<extern "sysv64" fn(u128) -> u128>,
    pub impl_slice_non_overlapping: unsafe extern "sysv64" fn(*mut u8, usize, *const u8, usize),
    pub impl_slice_in_out: unsafe extern "sysv64" fn(*mut u8, usize),
}

/// Fast custom op implementations.
#[derive(Debug, Copy, Clone)]
pub enum CustomOpFastImpl {
    Binary(CustomOpFastImplBinary),
    Unary(CustomOpFastImplUnary),
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
