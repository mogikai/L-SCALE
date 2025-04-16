#![allow(unused_variables)]
//! The x86 JIT implementation can make use of all registers, except RBP, since RBP stores the cpu context.
use crate::jit::api::{
    CustomOpImpl,
    JITGlobalRuntimeContext,
    JITPathChoice,
    JITResult,
    JITTaskRuntimeContext,
    JitError,
    MogiJit,
    PreludeSignature,
    TlbMissHelper,
};
use dynasmrt::x64::X64Relocation;
use memmap2::MmapOptions;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRVarNode,
    VarNodeId,
};
use mogi_sleigh::spec::{
    CustomOpId,
    ProcessorSpecification,
};
use mogi_utilities::addresses::EVAddr;
use std::cell::RefCell;
use std::ffi::c_void;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::jit::code_buffer::{
    AnonymousBlockId,
    CodeBuffer,
};
use crate::jit::x86::scaffolding::{
    build_epilogue,
    build_prelude,
};
use crate::jit::x86::x64emitter::{
    TlbMissCallback,
    X64AllocRequirements,
    X64Emitter,
};
use mogi_utilities::index_map::{
    MapKey,
    SecondaryMap,
};
use task_context::X86TaskContext;
use x64emitter::CustomOpCallback;

mod alloc_requirement;
mod registers;
mod scaffolding;
mod task_context;
#[cfg(test)]
mod test;
mod x64emitter;

macro_rules! assemble {
    ($asm:expr; $($t:tt)*) => {
        {
        #[allow(unused_imports)]
        use dynasmrt::DynasmApi;
        #[allow(unused_imports)]
        use dynasmrt::DynasmLabelApi;
        use dynasmrt::dynasm;
        dynasm!($asm
            ; .arch x64
            // The cpu context is stored in RBP for as long as we are inside the jited code.
            // RBP must never be used in any other way.
            // ; .alias cpu_context, rbp
            $($t)*
        )
    }
    };
}

use crate::jit::host::register::HostRegSet;
use crate::jit::regalloc::{
    RegAllocHostConstraints,
    RegisterAllocator,
};
use crate::jit::trace_jit;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::registers::xmm::XMMRegister;
use assemble;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;

pub struct X86MogiJit {
    code_buffer: CodeBuffer<X64Relocation>,
    processor_specification: Rc<RefCell<ProcessorSpecification>>,
    /// The jit prelude is the entry point of the emulator. The jir prelude is called with the
    /// context as first argument, and the block to execute as second argument (see [PreludeSignature])
    /// The prelude always lives in the first 1024 bits of the executable buffer.
    jit_prelude_block: AnonymousBlockId,
    jit_prelude: PreludeSignature,
    /// The jit epilogue is the exit point of the emulator. By default, blocks will chain to this
    /// special block, such that the execution returns to the main emulator loop.
    /// However, it is possible to directly link multiple jited blocks together, such that the execution
    /// remains in the jited code, skipping the main emulator loop. This is more efficient, but requires
    /// that the blocks have already been translated and are still valid.
    /// The epilogue will fully flush all registers into their corresponding varnodes.
    jit_epilogue_block: AnonymousBlockId,

    /// The global context pointer is baked into translated blocks for, e.g., custom op callbacks.
    /// Since the pointer is unmovable as lang as the jit exists, and the block cache is deleted
    /// along with the jit, this is safe to access.
    global_context: Pin<Box<X86GlobalContext>>,
}

/// The global jit context. Each task context contains a pointer to this context.
pub struct X86GlobalContext {
    custom_op_impls: SecondaryMap<CustomOpId, Option<CustomOpCallback<X86TaskContext>>>,
    tlb_miss_read_callback: Option<TlbMissCallback<X86TaskContext>>,
    tlb_miss_write_callback: Option<TlbMissCallback<X86TaskContext>>,
}

#[repr(transparent)]
/// The storage vec contains all registers and temporaries in a serialized form.
/// Each variable has an index and size.
/// TODO: We might need to compact the spaces in the spec, since they contain a lot of
///       unused areas. We could then possibly reduce the size to one or two pages.
#[derive(Debug)]
pub struct Space([u8; 0x3000]);

impl Space {
    fn new() -> Self {
        Self([0x41; 0x3000])
    }

    /// Get a reference to the data referred to by the provided slice.
    pub fn slice(
        &self,
        slice: JIRSlice,
    ) -> &[u8] {
        &self.0[slice.offset as usize..slice.end() as usize]
    }

    /// Get a mutable reference to the data referred to by the provided slice.
    pub fn slice_mut(
        &mut self,
        slice: JIRSlice,
    ) -> &mut [u8] {
        &mut self.0[slice.offset as usize..slice.end() as usize]
    }

    /// Get a reference to the varnode data referred to by the provided varnode.
    pub fn varnode(
        &self,
        varnode: &JIRVarNode,
    ) -> &[u8] {
        &self.0[varnode.offset as usize..varnode.end() as usize]
    }

    /// Get a mutable reference to the varnode data referred to by the provided varnode.
    pub fn varnode_mut(
        &mut self,
        varnode: &JIRVarNode,
    ) -> &mut [u8] {
        &mut self.0[varnode.offset as usize..varnode.end() as usize]
    }
}

impl Default for X86GlobalContext {
    fn default() -> Self {
        Self {
            custom_op_impls: Default::default(),
            tlb_miss_read_callback: Default::default(),
            tlb_miss_write_callback: Default::default(),
        }
    }
}

impl JITGlobalRuntimeContext for X86GlobalContext {}

impl X86MogiJit {
    // TODO: Make MMU generic (put behind trait)
    pub fn new(
        processor_specification: Rc<RefCell<ProcessorSpecification>>,
        mmu: Rc<RefCell<MogiMMU>>,
        debug: bool,
    ) -> Self {
        let mut code_buffer = CodeBuffer::new().unwrap();
        let (prelude_block, prelude) = build_prelude(&mut code_buffer, debug).unwrap();
        let epilogue_block = build_epilogue(&mut code_buffer).unwrap();
        Self {
            code_buffer,
            processor_specification,
            jit_prelude_block: prelude_block,
            jit_prelude: prelude,
            jit_epilogue_block: epilogue_block,
            global_context: Box::pin(X86GlobalContext::default()),
        }
    }
}

impl MogiJit for X86MogiJit {
    type RuntimeContext = X86TaskContext;
    type GlobalContext = X86GlobalContext;

    fn new_runtime_context(&mut self) -> Self::RuntimeContext {
        X86TaskContext::new(
            NonNull::new(&mut *self.global_context).expect("The pinned pointer must be non null per definition"),
        )
    }

    fn register_custom_op_impl<CB>(
        &mut self,
        op_id: CustomOpId,
        callback: CB,
    ) -> JITResult
    where
        CB: CustomOpImpl<Self::RuntimeContext>,
    {
        self.global_context.custom_op_impls[op_id] = Some(CustomOpCallback::new(callback));
        Ok(())
    }

    /// Register a callback that translates the provided [EVAddr] into an [HVAddr]
    /// in case the [TLB] does not contain a corresponding entry.
    /// The callback must perform a permission check and return an error, if the access would
    /// violate access permissions.
    /// This callback may add a [TLB] entry to facilitate faster translation for future accesses.
    /// There are two callbacks, one for read accesses and one for write accesses
    /// (see [Self::register_tlb_write_miss_callback])
    fn register_tlb_read_miss_callback<CB>(
        &mut self,
        callback: CB,
    ) -> JITResult
    where
        CB: TlbMissHelper<Self::RuntimeContext>,
    {
        self.global_context.tlb_miss_read_callback = Some(TlbMissCallback::new(callback));
        Ok(())
    }

    /// Register a callback that translates the provided [EVAddr] into an [HVAddr]
    /// in case the [TLB] does not contain a corresponding entry.
    /// The callback must perform a permission check and return an error, if the access would
    /// violate access permissions.
    /// See [Self::register_tlb_read_miss_callback].
    fn register_tlb_write_miss_callback<CB>(
        &mut self,
        callback: CB,
    ) -> JITResult
    where
        CB: TlbMissHelper<Self::RuntimeContext>,
    {
        self.global_context.tlb_miss_write_callback = Some(TlbMissCallback::new(callback));
        Ok(())
    }

    fn compile_target_block(
        &mut self,
        target_block: &TargetBasicBlock,
    ) -> JITResult {
        self.code_buffer
            .create_block(target_block.start(), target_block.fallthrough())?;
        // Fetch the offset of the epilogue block
        let epilogue_code = self
            .code_buffer
            .anonymous_block(self.jit_epilogue_block)
            .ptr()
            .expect("The epilogue block code must exist");
        let compiled_block = self
            .code_buffer
            .block_mut(target_block.start())
            .expect("We just created this block");

        // Set both exit succes and exit fail to the jit epilogue
        // TODO: Different block for exit_fail?
        unsafe {
            compiled_block.set_exit_success(epilogue_code);
            compiled_block.set_exit_alternate(epilogue_code);
            compiled_block.set_exit_failure(epilogue_code);
        }

        // We create a new emitter for the target architecture. The emitter will directly emit instructions
        // into the provided `compiled_block` for each instruction in the provided `target_block`.
        let spec = self.processor_specification.borrow();
        // TODO: Maybe it would make sense to reuse the alloc requirements buffers to avoid allocations?
        let alloc_requirements = X64AllocRequirements::new(&spec, target_block);

        let preferred_registers = HostRegSet::empty()
            .with(GPRegister::AX.into_host_reg())
            .with(GPRegister::BX.into_host_reg())
            .with(GPRegister::CX.into_host_reg())
            .with(GPRegister::DX.into_host_reg())
            .with(GPRegister::SI.into_host_reg())
            .with(GPRegister::DI.into_host_reg()) // scratch register
            .with(GPRegister::BP.into_host_reg())
            // .with(GPRegister::SP.into_host_reg()) // sp is reserved
            .with(XMMRegister::XMM0.into_host_reg())
            .with(XMMRegister::XMM1.into_host_reg())
            .with(XMMRegister::XMM2.into_host_reg())
            .with(XMMRegister::XMM3.into_host_reg())
            .with(XMMRegister::XMM4.into_host_reg())
            .with(XMMRegister::XMM5.into_host_reg())
            .with(XMMRegister::XMM6.into_host_reg())
            .with(XMMRegister::XMM7.into_host_reg()); // scratch register

        // Mark the registers with longer encodings as non_preferred.
        // For most architectures, this should only impact code size and not performance.
        let non_preferred_registers = HostRegSet::empty()
            .with(GPRegister::R8.into_host_reg())
            .with(GPRegister::R9.into_host_reg())
            .with(GPRegister::R10.into_host_reg())
            .with(GPRegister::R11.into_host_reg())
            .with(GPRegister::R12.into_host_reg())
            .with(GPRegister::R13.into_host_reg())
            .with(GPRegister::R14.into_host_reg())
            .with(GPRegister::R15.into_host_reg())
            .with(XMMRegister::XMM8.into_host_reg())
            .with(XMMRegister::XMM9.into_host_reg())
            .with(XMMRegister::XMM10.into_host_reg())
            .with(XMMRegister::XMM11.into_host_reg())
            .with(XMMRegister::XMM12.into_host_reg())
            .with(XMMRegister::XMM13.into_host_reg())
            .with(XMMRegister::XMM14.into_host_reg())
            .with(XMMRegister::XMM15.into_host_reg());

        // And then start the register allocator for the block and target architecture.
        let regalloc = RegisterAllocator::new(
            &target_block,
            &alloc_requirements,
            RegAllocHostConstraints::new(preferred_registers, non_preferred_registers)
                .with_scratch(GPRegister::DI.into_host_reg())
                .with_scratch(XMMRegister::XMM7.into_host_reg()),
        );
        let allocations = regalloc.run()?;

        // And finally we emit the instructions, using the allocations we just calculated.
        let emitter =
            alloc_requirements.make_emitter(&allocations, &self.global_context, &spec, compiled_block, target_block);
        emitter.emit_block()?;
        self.code_buffer.write_block(target_block.start())?;
        Ok(())
    }

    fn chain_blocks(
        &mut self,
        origin_block_address: EVAddr,
        target_block_address: EVAddr,
        path: JITPathChoice,
    ) -> JITResult {
        let target_block_ptr = self
            .code_buffer
            .block(target_block_address)
            .ok_or(JitError::BlockNotFound(target_block_address))?
            .ptr()
            .ok_or(JitError::BlockNotFound(target_block_address))?;
        let origin_block = self
            .code_buffer
            .block_mut(origin_block_address)
            .ok_or(JitError::BlockNotFound(origin_block_address))?;
        // TODO: Safety?
        match path {
            JITPathChoice::Main => unsafe {
                origin_block.set_exit_success(target_block_ptr);
            },
            JITPathChoice::Alternate => unsafe {
                origin_block.set_exit_alternate(target_block_ptr);
            },
        }
        origin_block.commit()?;
        Ok(())
    }

    fn execute(
        &self,
        context: &mut Self::RuntimeContext,
        block: EVAddr,
    ) -> Result<(), JitError> {
        let block = self
            .code_buffer
            .block(block)
            .ok_or(JitError::BlockNotFound(block))?
            .ptr()
            .ok_or_else(|| -> JitError { todo!("Return error for missing jit code") })?;
        // SAFETY: Context is a valid pointer to a runtime context. The context outlives this function call,
        //         since it is passed in from outside the function.
        //         The pointer to the executable memory of the block is valid, since we just fetched it from the
        //         code buffer, and it will remain valid for the lifetime of this function.
        //         TODO: What about self-modifying code?
        unsafe {
            (self.jit_prelude)(context as *mut _ as *mut c_void, block);
        }
        Ok(())
    }

    fn is_cached(
        &self,
        block: EVAddr,
    ) -> bool {
        self.code_buffer.block(block).is_some()
    }

    fn instruction_pointer(
        &self,
        context: &Self::RuntimeContext,
    ) -> Result<EVAddr, JitError> {
        let pc = EVAddr::from(u64::from_le_bytes(
            context
                .slice(
                    self.processor_specification
                        .borrow()
                        .program_counter_slice(),
                )?
                .try_into()
                .unwrap(),
        ));
        Ok(pc)
    }

    fn set_instruction_pointer(
        &self,
        context: &mut Self::RuntimeContext,
        ip: EVAddr,
    ) -> Result<EVAddr, JitError> {
        let pc = context.slice_mut(
            self.processor_specification
                .borrow()
                .program_counter_slice(),
        )?;
        let old_pc = EVAddr::from(u64::from_le_bytes(pc.try_into().unwrap()));
        pc.copy_from_slice(u64::from(ip).to_le_bytes().as_slice());
        Ok(old_pc)
    }

    fn varnode<'b>(
        &self,
        context: &'b Self::RuntimeContext,
        varnode: VarNodeId,
    ) -> Result<&'b [u8], JitError> {
        let spec = self.processor_specification.borrow();
        let varnode = spec.varnode_by_id(varnode).unwrap();
        Ok(context.spaces()[varnode.space_id.to_index()].varnode(varnode))
    }

    fn varnode_mut<'b>(
        &self,
        context: &'b mut Self::RuntimeContext,
        varnode: VarNodeId,
    ) -> Result<&'b mut [u8], JitError> {
        let spec = self.processor_specification.borrow();
        let varnode = spec.varnode_by_id(varnode).unwrap();
        Ok(context.spaces_mut()[varnode.space_id.to_index()].varnode_mut(varnode))
    }
}

impl From<std::io::Error> for JitError {
    fn from(value: std::io::Error) -> Self {
        // TODO: Pass through error message
        JitError::IOError
    }
}
