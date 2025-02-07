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
    RuntimeError,
    TlbMissHelper,
};
use dynasmrt::dynasm;
use dynasmrt::x64::X64Relocation;
use memmap2::MmapOptions;
use mogi_sleigh::spec::jir::jcode::{
    JIRInstruction,
    JIROperation,
};
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
use tracing::trace;

use crate::jit::code_buffer::{
    AnonymousBlockId,
    CodeBuffer,
    CompiledBasicBlock,
};
use crate::jit::x86::emit::TlbMissCallback;
use crate::jit::x86::scaffolding::{
    build_epilogue,
    build_prelude,
};
use emit::CustomOpCallback;
use mogi_utilities::index_map::SecondaryMap;
use registers::{
    RegisterAllocation,
    RegisterTracker,
};
use task_context::X86TaskContext;

mod emit;
mod regalloc;
mod registers;
mod scaffolding;
mod task_context;
#[cfg(test)]
mod test;

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
            ; .alias cpu_context, rbp
            $($t)*
        )
    }
    };
}

use assemble;
use mogi_mem::mmu::MogiMMU;

pub struct X86MogiJit {
    code_buffer: CodeBuffer<X64Relocation>,
    processor_specification: Rc<RefCell<ProcessorSpecification>>,
    current_block: Option<EVAddr>,
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
    /// The register tracker keeps track of the registers used in the currently constructed block.
    /// If all registers are occupied, one of the registers must be spilled into memory, to free
    /// up space for the current operation.
    register_tracker: RegisterTracker,
    mmu: Rc<RefCell<MogiMMU>>,
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
        Self([0; 0x3000])
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
        let map = MmapOptions::new().len(4096 * (2 << 16)).map_anon().unwrap();
        let mut code_buffer = CodeBuffer::new().unwrap();
        let (prelude_block, prelude) = build_prelude(&mut code_buffer, debug).unwrap();
        let epilogue_block = build_epilogue(&mut code_buffer).unwrap();
        Self {
            code_buffer,
            processor_specification,
            current_block: Default::default(),
            jit_prelude_block: prelude_block,
            jit_prelude: prelude,
            jit_epilogue_block: epilogue_block,
            register_tracker: Default::default(),
            mmu,
            global_context: Box::pin(X86GlobalContext::default()),
        }
    }
}

impl X86MogiJit {
    /// Directly chain a target address to the block at the provided EVAddr.
    fn chain_blocks_direct(
        &mut self,
        origin_block_address: EVAddr,
        target_address: *const u8,
    ) -> JITResult {
        todo!()
        // // We need to take the block out of the cache, in order to modify it.
        // let mut origin = self.jit_cache.take(origin_block_address)?;
        // let Some(epilogue_offset) = origin.jmp_epilogue_offset else {
        //     Err(JitError::NoEpilogue)?
        // };
        // // The origin address is after the jump instruction, hence we add 5 here.
        // // See the intel manual for relative jmp size with 32 bit argument.
        // let orign_address = unsafe { origin.code.ptr(epilogue_offset).add(5) };
        // let jmp_offset = i32::try_from(unsafe { target_address.offset_from(orign_address) }).unwrap();
        // // We know this must be safe, since the offset points into the memory starting at code.as_ptr
        // let offset = unsafe {
        //     origin
        //         .code
        //         .ptr(epilogue_offset)
        //         .offset_from(origin.code.as_ptr()) as usize
        // };
        // let mut new_jmp = MogiAssembler::<X64Relocation>::new(0)?;
        // dynasm!(new_jmp
        // ; jmp jmp_offset as _
        // );
        // let new_jmp = new_jmp.finalize().unwrap();
        // let mut code = origin.code.make_mut()?;
        // let mut code = origin.code.make_mut()?;
        // // Replace the jmp instruction with a new one pointing to the new target address
        // code[offset..offset + new_jmp.len()].copy_from_slice(&new_jmp);
        // origin.code = code.make_exec()?;
        // // Done! We can now put the block back into the cache.
        // self.jit_cache.populate(origin);
        // Ok(())
    }

    // fn emit_host_load(
    //     &mut self,
    //     ty: (),
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    // ) -> Result<Self::Value, JitError> {
    //     todo!()
    // }
    //
    // fn emit_host_store(
    //     &mut self,
    //     ty: (),
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    // ) -> Result<Self::Value, JitError> {
    //     todo!()
    // }

    // fn emit_load_fast_path(
    //     &mut self,
    //     target_register: u8,
    // ) -> JITResult {
    //     let block = self.current_block()?;
    //     dynasm!(block.assembler
    //         ; mov Rq(target_register), 0
    //     );
    //     Ok(())
    // }

    #[inline]
    fn block_mut(&mut self) -> Result<&mut CompiledBasicBlock<X64Relocation>, JitError> {
        Ok(self
            .code_buffer
            .block_mut(self.current_block.ok_or(JitError::NoActiveBlock)?)
            .expect("The block must exist"))
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

    fn new_target_block(
        &mut self,
        evaddr: EVAddr,
        successor: Option<EVAddr>,
    ) -> JITResult {
        if self.current_block.is_some() {
            todo!("Return error: 'Tried to create new block without finalizing the previous block'")
        }
        self.code_buffer.create_block(evaddr, successor)?;
        self.current_block = Some(evaddr);
        // Fetch the offset of the epilogue block
        let epilogue_code = self
            .code_buffer
            .anonymous_block(self.jit_epilogue_block)
            .ptr()
            .expect("The epilogue block code must exist");
        let block = self
            .code_buffer
            .block_mut(evaddr)
            .expect("We just created this block");

        // Set both exit succes and exit fail to the jit epilogue
        // TODO: Different block for exit_fail?
        unsafe {
            block.set_exit_success(epilogue_code);
            block.set_exit_alternate(epilogue_code);
            block.set_exit_failure(epilogue_code);
        }
        Ok(())
    }

    fn emit_jinst(
        &mut self,
        jinst: &JIRInstruction,
    ) -> JITResult {
        trace!("Emitting {jinst}");
        // TODO: Maybe we can "allocate" the registers ahead of time and provide some hints
        //       or something similar. Allocating the registers when having a complete
        //       view on all relevant instructions may be more efficient.
        let output = jinst.output.ok_or(RuntimeError::MissingOutput);
        let input_a = jinst.input_a.ok_or(RuntimeError::MissingInput);
        let input_b = jinst.input_b.ok_or(RuntimeError::MissingInput);
        match jinst.operation {
            JIROperation::BlockStart(block_id) => return Ok(()),
            JIROperation::BlockEnd(block_id) => return Ok(()),
            JIROperation::InstructionStart(addr) => return Ok(()),
            JIROperation::InstructionEnd(addr) => return Ok(()),
            JIROperation::Copy => return self.emit_copy(output?, input_a?),
            JIROperation::Load(space_id) => return self.emit_load(output?, input_a?, space_id),
            JIROperation::Store(space_id) => return self.emit_store(input_a?, space_id, input_b?),
            JIROperation::IntAdd => return self.emit_add(output?, input_a?, input_b?),
            JIROperation::IntSubtract => return self.emit_subtract(output?, input_a?, input_b?),
            JIROperation::IntMul => return self.emit_multiply(output?, input_a?, input_b?),
            JIROperation::IntDiv => return self.emit_divide(output?, input_a?, input_b?),
            JIROperation::IntSDiv => return self.emit_signed_divide(output?, input_a?, input_b?),
            JIROperation::IntRemainder => return self.emit_remainder(output?, input_a?, input_b?),
            JIROperation::IntSRemainder => return self.emit_signed_remainder(output?, input_a?, input_b?),
            JIROperation::IntLeft => {}
            JIROperation::IntRight => {}
            JIROperation::IntSRight => {}
            JIROperation::IntOr => return self.emit_bitwise_or(output?, input_a?, input_b?),
            JIROperation::IntAnd => return self.emit_bitwise_and(output?, input_a?, input_b?),
            JIROperation::IntXor => return self.emit_bitwise_xor(output?, input_a?, input_b?),
            JIROperation::IntSLess => {}
            JIROperation::IntSGreater => {}
            JIROperation::IntSLessEqual => {}
            JIROperation::IntSGreaterEqual => {}
            JIROperation::IntLess => {}
            JIROperation::IntGreater => {}
            JIROperation::IntLessEqual => {}
            JIROperation::IntGreaterEqual => {}
            JIROperation::IntEquals => {}
            JIROperation::IntNotEquals => {}
            JIROperation::BitNegate => return self.emit_bitwise_negate(output?, input_a?),
            JIROperation::IntNegate => return self.emit_negate(output?, input_a?),
            JIROperation::FloatDiv => {}
            JIROperation::FloatMul => {}
            JIROperation::FloatAdd => {}
            JIROperation::FloatSubtract => {}
            JIROperation::FloatLess => {}
            JIROperation::FloatGreater => {}
            JIROperation::FloatLessEqual => {}
            JIROperation::FloatGreaterEqual => {}
            JIROperation::FloatEquals => {}
            JIROperation::FloatNotEquals => {}
            JIROperation::FloatNegate => {}
            JIROperation::BoolOr => return self.emit_boolean_or(output?, input_a?, input_b?),
            JIROperation::BoolAnd => return self.emit_boolean_and(output?, input_a?, input_b?),
            JIROperation::BoolXor => return self.emit_boolean_xor(output?, input_a?, input_b?),
            JIROperation::BoolNegate => return self.emit_boolean_negate(output?, input_a?),
            JIROperation::CallDirect => {}
            JIROperation::CallIndirect => {}
            JIROperation::ReturnDirect => {}
            JIROperation::ReturnIndirect => {}
            JIROperation::GotoDirect => return self.emit_goto_direct(input_a?),
            JIROperation::GotoIndirect => return self.emit_goto_indirect(input_a?),
            JIROperation::GotoDirectIf => return self.emit_goto_direct_if(input_a?, input_b?),
            JIROperation::GotoIndirectIf => {}
            JIROperation::JCodeGoto(block_id) => {}
            JIROperation::JCodeGotoIf(block_id) => {}
            JIROperation::PopCount => {}
            JIROperation::LZCount => {}
            JIROperation::ZeroExtend => return self.emit_zero_extend(output?, input_a?),
            JIROperation::SignExtend => {}
            JIROperation::Carry => {}
            JIROperation::SignCarry => {}
            JIROperation::SignBorrow => {}
            JIROperation::Nan => {}
            JIROperation::Abs => {}
            JIROperation::Sqrt => {}
            JIROperation::IntToFloat => {}
            JIROperation::FloatToFloat => {}
            JIROperation::Trunc => {}
            JIROperation::Ceil => {}
            JIROperation::Floor => {}
            JIROperation::Round => {}
            JIROperation::CustomOp { op_id, ref extra_args } => {
                return self.emit_custom_op_call(op_id, output, input_a, input_b, extra_args.as_slice());
            }
        }
        todo!("Instruction not yet implemented: {jinst}")
    }

    fn emit_flush_slices(&mut self) -> JITResult {
        // Iterate over the loaded slices and flush them to memory
        // TODO: Do not insert flush instructions for dead registers.
        for (register, slice) in self.register_tracker.reset_gp().iter() {
            if let RegisterAllocation::OccupiedDirty(slice) = slice {
                self.emit_flush_register_to_slice(register, *slice)?;
            }
        }
        Ok(())
    }

    // fn emit_load(
    //     &mut self,
    //     ty: (),
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    // ) -> Result<Self::Value, JitError> {
    //     todo!()
    // }
    //
    // fn emit_store(
    //     &mut self,
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    //     value: Self::Value,
    // ) -> JITResult {
    //     todo!()
    // }

    // fn emit_ret(&mut self) -> JITResult {
    //     let block = self.current_block()?;
    //     dynasm!(block.assembler
    //         ; ret
    //     );
    //     Ok(())
    // }

    // fn emit_pop(
    //     &mut self,
    //     var: Self::Value,
    // ) -> JITResult {
    //     // TODO: Every basic block has "outputs" and "inputs"
    //     //       "outputs" are simply variables that have been written to during the blocks execution.
    //     //       "inputs" are variables that are read from, before they are written to during the blocks execution.
    //     // TODO: self.allocate_registers()?
    //     //       Need to check if var is already a register (e.g. context is fixed as a register)
    //     //       If it is not a register, it needs to be loaded from the cpu state
    //     todo!()
    //     // TODO: self.sync_registers()?
    //     //       we might need to write registers back to the cpu state, depending on if they
    //     //       are overwritten before the block ends or not.
    //     //       Effectively, all "output" registers need to be written back.
    //     //       Whether or not a variable is "dead" or an "output", is set by the MIR optimizer.
    //     //       The state is stored in the variable struct.
    //     //       In order to do this, we need to write a store instruction to host assembly.
    // }

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
        // // This is safe, since we only relocate the target block if we need to update the code.
        // // This requires unchaining the block, which will remove this address and replace it with
        // // the epilogue.
        // let target_address = unsafe {
        //     self.jit_cache
        //         .get_block_code(target_block_address)?
        //         .raw_ptr()
        // };
        // self.chain_blocks_direct(origin_block_address, target_address)
    }

    fn finalize_block(&mut self) -> JITResult {
        if self.current_block.is_none() {
            todo!("Return error: 'Cannot finalize because no block was started'")
        };
        match self.block_mut()?.fallthrough() {
            None => {
                // No successor specified. This means the processor is halting or something similar.
                // Jump to failure exit.
                // TODO: Write exception to runtime context.
                assemble!(self.block_mut()?.assembler();
                    // Intentionally trigger an exception, since this case is a todo
                    ; mov rax, [0x0]
                );
            }
            Some(next) => {
                // Write the pc before we exit the block to make it visible to the jit.
                // TODO: This could maybe be optimized such that we do not write the pc on chained blocks?
                self.emit_write_pc(u64::from(next).into())?;
                self.emit_flush_slices()?;
                assemble!(self.block_mut()?.assembler();
                    ; jmp ->exit_success
                );
            }
        }
        let block_addr = self.current_block.take().expect("The block must exist");
        self.code_buffer.write_block(block_addr)?;
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
        Ok(context.spaces()[usize::from(varnode.space_id)].varnode(varnode))
    }

    fn varnode_mut<'b>(
        &self,
        context: &'b mut Self::RuntimeContext,
        varnode: VarNodeId,
    ) -> Result<&'b mut [u8], JitError> {
        let spec = self.processor_specification.borrow();
        let varnode = spec.varnode_by_id(varnode).unwrap();
        Ok(context.spaces_mut()[usize::from(varnode.space_id)].varnode_mut(varnode))
    }
}

impl From<std::io::Error> for JitError {
    fn from(value: std::io::Error) -> Self {
        // TODO: Pass through error message
        JitError::IOError
    }
}
