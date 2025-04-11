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
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::jir_block::JIRBlockId;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

pub enum ProgramCounter {
    Register(HostReg),
    Constant(JIRConst),
}

enum JumpType {
    Direct,
    Indirect,
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

    pub fn goto_indirect_if_requirements(
        requirement: &mut AllocRequirement,
        condition: JIRValue,
        pc: JIRSlice,
        target: JIRValue,
        space: SpaceId,
    ) {
        Self::load_requirements(requirement, pc, target, space);
        condition.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
    }

    pub fn jcode_goto_requirements(
        requirement: &mut AllocRequirement,
        block_id: JIRBlockId,
    ) {
    }

    pub fn jcode_goto_if_requirements(
        requirement: &mut AllocRequirement,
        condition: JIRValue,
        block_id: JIRBlockId,
    ) {
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
        self.emit_goto(inst, target, JumpType::Direct)
    }

    /// Emit a goto instruction that jumps to the address loaded from memory located at `target`.
    /// For directly jumping to `target` instead of the address at that location, see [emit_goto_direct](Self::emit_goto_direct)
    pub fn emit_goto_indirect(
        &mut self,
        inst: InstId,
        target: JIRValue,
    ) -> JITResult {
        self.emit_goto(inst, target, JumpType::Indirect)
    }

    fn emit_goto(
        &mut self,
        inst: InstId,
        target: JIRValue,
        jump_type: JumpType,
    ) -> JITResult {
        self.block.clear_fallthrough();

        self.emit_write_pc(inst, target, jump_type)?;

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
        self.emit_goto_if(inst, target, condition, JumpType::Direct)
    }

    pub fn emit_goto_indirect_if(
        &mut self,
        inst: InstId,
        target: JIRValue,
        condition: JIRValue,
    ) -> JITResult {
        self.emit_goto_if(inst, target, condition, JumpType::Indirect)
    }

    pub fn emit_goto_if(
        &mut self,
        inst: InstId,
        target: JIRValue,
        condition: JIRValue,
        jump_type: JumpType,
    ) -> JITResult {
        let condition_reg = match condition {
            JIRValue::Slice(slice) => {
                let mut condition_op_id = if target.is_slice() { 1 } else { 0 };
                if let JumpType::Indirect = jump_type {
                    // If the jump is indirect, we load the pc directly. The load is delegated to the load
                    // implementation, which always requests the output, in this case the pc.
                    // The target is only requested if it is a slice just like in the direct jump case.
                    condition_op_id += 1;
                }
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
        self.check_condition(condition_reg, condition.size())?;
        assemble!(self.block.assembler();
            // Jump over load of alternate if condition_reg == 0
            ; jnz >alternate
            ; jmp >proceed
            ; alternate:
        );

        // If the condition is met, write the alternate target pc and then jump to the alternate exit routine.
        self.emit_write_pc(inst, target, jump_type)?;

        // Emit all after directives now, since we jump afterward.
        self.emit_directives(self.allocations.directives().after_inst_directives(inst))?;
        assemble!(self.block.assembler();
            ; jmp ->exit_alternate
            // Jump here if condition is not met, thereby skipping the pc store
            ; proceed:
        );
        Ok(())
    }

    fn emit_write_pc(
        &mut self,
        inst: InstId,
        target: JIRValue,
        jump_type: JumpType,
    ) -> JITResult {
        match jump_type {
            JumpType::Direct => {
                let alternate = match target {
                    JIRValue::Slice(target) => ProgramCounter::Register(reg!(self.allocations, inst, 0)),
                    JIRValue::Const(target) => ProgramCounter::Constant(target),
                };
                self.emit_load_pc_from_reg(alternate)
            }
            JumpType::Indirect => self.emit_load_pc_from_mem(inst, target),
        }
    }

    /// Write the provided address to the program counter slice.
    pub fn emit_load_pc_from_reg(
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
    pub fn emit_load_pc_from_mem(
        &mut self,
        inst: InstId,
        address: JIRValue,
    ) -> JITResult {
        let pc = self.processor_spec.program_counter_slice();
        let space = self.processor_spec.default_space();
        self.emit_load(inst, pc, address, space)
    }

    pub fn emit_jcode_goto(
        &mut self,
        inst: InstId,
        block_id: JIRBlockId,
    ) -> JITResult {
        let label = self.block.dynamic_label(block_id);
        assemble!(self.block.assembler();
            ; jmp =>label
        );
        Ok(())
    }

    pub fn emit_jcode_goto_if(
        &mut self,
        inst: InstId,
        condition: JIRValue,
        block_id: JIRBlockId,
    ) -> JITResult {
        let condition_reg = match condition {
            JIRValue::Slice(slice) => reg!(self.allocations, inst, 0),
            JIRValue::Const(value) => {
                let scratch = self.allocations.scratch_register(RegClass::Int);
                self.load_const(scratch, value)?;
                scratch
            }
        };
        let label = self.block.dynamic_label(block_id);
        self.check_condition(condition_reg, condition.size())?;
        assemble!(self.block.assembler();
            // Jump to alternate jir block if condition_reg != 0
            ; jnz =>label
        );
        Ok(())
    }

    pub fn check_condition(
        &mut self,
        condition_reg: HostReg,
        condition_size: JIRSize,
    ) -> JITResult {
        match condition_size {
            1 => assemble!(self.block.assembler();
                // condition_reg == 0?
                ; test Rb(condition_reg.hw_enc()), Rb(condition_reg.hw_enc())
            ),
            2 => assemble!(self.block.assembler();
                // condition_reg == 0?
                ; test Rw(condition_reg.hw_enc()), Rw(condition_reg.hw_enc())
            ),
            4 => assemble!(self.block.assembler();
                // condition_reg == 0?
                ; test Rd(condition_reg.hw_enc()), Rd(condition_reg.hw_enc())
            ),
            8 => assemble!(self.block.assembler();
                // condition_reg == 0?
                ; test Rq(condition_reg.hw_enc()), Rq(condition_reg.hw_enc())
            ),
            _ => todo!("Unsupported condition size"),
        }
        Ok(())
    }
}
