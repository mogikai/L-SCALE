use crate::jit::api::{
    JITResult,
    MogiJit,
};
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use crate::tlb::PageMetaInfo;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRValue,
};

impl<PageDataT> X86MogiJit<PageDataT>
where
    PageDataT: PageMetaInfo,
{
    /// Emit a goto instruction that jumps to the target address.
    /// Goto is implemented by writing the desired jump target to the program counter and then jumping to the
    /// exit_success target. This will either return into the jit main loop, where the next block is fetched,
    /// or it will directly jump to the next block, if a block is chained.
    pub fn emit_goto_direct(
        &mut self,
        target: JIRValue,
    ) -> JITResult {
        self.block_mut()?.clear_fallthrough();
        self.emit_write_pc(target)?;
        self.emit_flush_slices()?;
        // We jump to the exit label. This will either jump to the epilogue or to the chained block.
        // TODO: Maybe we need to check if an external goto was already performed and then return an error?
        assemble!(self.block_mut()?.assembler();
            ; jmp ->exit_success
        );
        Ok(())
    }

    /// Emit a goto instruction that jumps to the address loaded from memory located at `target`.
    /// For directly jumping to `target` instead of the address at that location, see [emit_goto_direct](Self::emit_goto_direct)
    pub fn emit_goto_indirect(
        &mut self,
        target: JIRValue,
    ) -> JITResult {
        self.emit_load_pc(target)?;
        self.emit_flush_slices()?;
        assemble!(self.block_mut()?.assembler();
            ; jmp ->exit_success
        );
        Ok(())
    }

    pub fn emit_goto_direct_if(
        &mut self,
        target: JIRValue,
        condition: JIRValue,
    ) -> JITResult {
        let condition_gpr = match condition {
            JIRValue::Slice(slice) => self.load_slice_gpr(slice, SliceType::Input),
            JIRValue::Const(value) => {
                let temp_gpr = self.get_temp_gpr();
                self.emit_load_const_to_gpr(temp_gpr, value)?;
                temp_gpr
            }
        };
        let Some(successor) = self.block_mut()?.fallthrough() else {
            todo!("conditional branch requires the block to have a successor.")
        };
        assemble!(self.block_mut()?.assembler();
            ; cmp Rq(condition_gpr as u8), 0
        );
        // Write the non-branching pc and jump to the normal exit routine if the condition is not met.
        // TODO: Conditional move of target if the condition is true. Emit flush code only once.
        self.emit_write_pc(JIRConst::new(u64::from(successor).into(), target.size()).into())?;
        self.emit_flush_slices()?;
        assemble!(self.block_mut()?.assembler();
            ; je ->exit_success
        );
        // If the condition is met, write the alternate target pc and then jump to the alternate exit routine.
        self.emit_write_pc(target)?;
        self.emit_flush_slices()?;
        assemble!(self.block_mut()?.assembler();
            ; jmp ->exit_alternate
        );
        Ok(())
    }

    /// Write the provided address to the program counter.
    pub fn emit_write_pc(
        &mut self,
        address: JIRValue,
    ) -> JITResult {
        let pc = self
            .processor_specification
            .borrow()
            .program_counter_slice();
        self.emit_copy(pc, address)
    }

    /// Load the program counter from the memory location pointed to by `address`.
    pub fn emit_load_pc(
        &mut self,
        address: JIRValue,
    ) -> JITResult {
        let spec = self.processor_specification.borrow();
        let pc = spec.program_counter_slice();
        let space = spec.default_space();
        drop(spec);
        self.emit_load(pc, address, space)
    }
}
