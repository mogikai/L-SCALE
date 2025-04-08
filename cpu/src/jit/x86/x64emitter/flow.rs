use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::x64emitter::helpers::reg;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::jit::x86::{
    X86GlobalContext,
    assemble,
};
use mogi_sleigh::spec::SpaceId;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

pub enum ProgramCounter {
    Register(HostReg),
    Constant(JIRConst),
}

impl From<JIRConst> for ProgramCounter {
    fn from(value: JIRConst) -> Self {
        ProgramCounter::Constant(value)
    }
}

impl X64AllocRequirements {
    pub fn goto_direct_requirements(
        requirement: &mut AllocRequirement,
        target: JIRValue,
    ) {
        target.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
    }

    pub fn goto_indirect_requirements(
        requirement: &mut AllocRequirement,
        pc: JIRSlice,
        target: JIRValue,
        space: SpaceId,
    ) {
        Self::load_requirements(requirement, pc, target, space);
    }

    pub fn goto_direct_if_requirements(
        requirement: &mut AllocRequirement,
        condition: JIRValue,
        target: JIRValue,
    ) {
        target.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
        condition.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
    }
}

impl X64Emitter<'_> {
    /// Emit a goto instruction that jumps to the target address.
    /// Goto is implemented by writing the desired jump target to the program counter and then jumping to the
    /// exit_success target. This will either return into the jit main loop, where the next block is fetched,
    /// or it will directly jump to the next block, if a block is chained.
    pub fn emit_goto_direct(
        &mut self,
        inst: InstId,
        target: JIRValue,
    ) -> JITResult {
        self.block.clear_fallthrough();
        let pc = match target {
            JIRValue::Slice(address) => ProgramCounter::Register(reg!(self.allocations, inst, 0)),
            JIRValue::Const(address) => ProgramCounter::Constant(address),
        };
        self.emit_write_pc(pc)?;
        // TODO: Check overlap of inputs for next block and adjust allocations instead of saving slices
        //       if the blocks are chained?
        // Saving slices is not necessary, since the register allocator will have emitted spill directives
        // for all previous writes.
        // self.save_loaded_slices()?;

        // Emit all after directives now, since we jump afterward.
        self.emit_directives(self.allocations.directives().after_inst_directives(inst))?;

        // We jump to the exit label. This will either jump to the epilogue or to the chained block.
        assemble!(self.block.assembler();
            ; jmp ->exit_success
        );
        Ok(())
    }

    /// Emit a goto instruction that jumps to the address loaded from memory located at `target`.
    /// For directly jumping to `target` instead of the address at that location, see [emit_goto_direct](Self::emit_goto_direct)
    pub fn emit_goto_indirect(
        &mut self,
        inst: InstId,
        target: JIRValue,
    ) -> JITResult {
        self.emit_load_pc(inst, target)?;

        // Emit all after directives now, since we jump afterward.
        self.emit_directives(self.allocations.directives().after_inst_directives(inst))?;

        assemble!(self.block.assembler();
            ; jmp ->exit_success
        );
        Ok(())
    }

    pub fn emit_goto_direct_if(
        &mut self,
        inst: InstId,
        target: JIRValue,
        condition: JIRValue,
    ) -> JITResult {
        let condition_reg = match condition {
            JIRValue::Slice(slice) => {
                let condition_op_id = if target.is_slice() { 1 } else { 0 };
                reg!(self.allocations, inst, condition_op_id)
            }
            JIRValue::Const(value) => {
                let scratch = self.allocations.scratch_register(RegClass::Int);
                self.load_const(scratch, value)?;
                scratch
            }
        };
        let Some(fallthrough) = self.block.fallthrough() else {
            panic!("Conditional branch requires the block to have a fallthrough.")
        };
        assemble!(self.block.assembler();
            // condition_reg == 0?
            ; test Rq(condition_reg.hw_enc()), Rq(condition_reg.hw_enc())
            // Jump over load of alternate if condition_reg == 0
            ; jnz >alternate
            ; jmp >proceed
            ; alternate:
        );
        // If the condition is met, write the alternate target pc and then jump to the alternate exit routine.
        let alternate = match target {
            JIRValue::Slice(target) => ProgramCounter::Register(reg!(self.allocations, inst, 0)),
            JIRValue::Const(target) => ProgramCounter::Constant(target),
        };
        self.emit_write_pc(alternate)?;

        // Emit all after directives now, since we jump afterward.
        self.emit_directives(self.allocations.directives().after_inst_directives(inst))?;
        assemble!(self.block.assembler();
            ; jmp ->exit_alternate
            // Jump here if condition is not met, thereby skipping the pc store
            ; proceed:
        );
        Ok(())
    }

    /// Write the provided address to the program counter slice.
    pub fn emit_write_pc(
        &mut self,
        address: ProgramCounter,
    ) -> JITResult {
        let pc = self.processor_spec.program_counter_slice();
        match address {
            ProgramCounter::Register(address) => self.write_int_reg_to_mem(pc, address),
            ProgramCounter::Constant(address) => self.write_const_to_mem(pc, address),
        }
    }

    /// Load the program counter from the memory location pointed to by `address`.
    pub fn emit_load_pc(
        &mut self,
        inst: InstId,
        address: JIRValue,
    ) -> JITResult {
        let pc = self.processor_spec.program_counter_slice();
        let space = self.processor_spec.default_space();
        self.emit_load(inst, pc, address, space)
    }
}
