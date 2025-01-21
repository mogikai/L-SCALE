use crate::jit::api::{
    JITResult,
    MogiJit,
};
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::values::JIRValue;

impl X86MogiJit {
    pub fn emit_goto_direct(
        &mut self,
        target: JIRValue,
    ) -> JITResult {
        self.emit_write_pc(target)?;
        self.emit_flush_slices()?;
        // We jump to the exit label. This will either jump to the epilogue or to the chained block.
        // TODO: Maybe we need to check if an external goto was already performed and then return an error?
        assemble!(self.block_mut()?.assembler();
            ; jmp ->exit_success
        );
        Ok(())
    }

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
