mod instruction;
pub mod requirements;
pub mod result;

use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
    HostRegMap,
    HostRegSet,
};
use crate::jit::regalloc::requirements::{
    OperandConstraint,
    OperandKind,
    OperandPos,
    OperandRequest,
};
use crate::jit::trace_jit;
use mogi_sleigh::spec::jir::basic_block::{
    InstId,
    TargetBasicBlock,
};
use mogi_sleigh::spec::jir::jir_block::JIRBlockId;
use mogi_sleigh::spec::jir::values::JIRSlice;
use requirements::EmitterAllocRequirements;
use result::AllocatorResult;

#[derive(Debug)]
pub struct RegAllocHostConstraints {
    pub usable_registers: HostRegSet,
}

pub struct RegisterAllocator<'a, E> {
    /// The target basic block. It contains the instructions and JirBlocks that the allocator performs allocations for.
    target_block: &'a TargetBasicBlock,

    /// The emitter. It specifies the operand and register requirements for each instruction.
    /// The emitter will later use the allocation result to emit the actual assembly.
    emitter_requirements: &'a E,

    allocations: AllocatorResult,

    /// Maps the [HostReg]
    loaded_slices: HostRegMap<JIRSlice>,

    /// TODO: Add the appropriate env constraints. We probably need to be able to exclude some registers for example.
    ///       It might also make sense to define the set of usable registers this way.
    host_constraints: RegAllocHostConstraints,

    /// Keeps track of the available registers before and after an instruction.
    /// This set of registers is reset for each instruction according to
    /// the instructions' and environments' constraints.
    available_registers: AvailableRegisters,
}

/// A struct to keep track of the registers that are available before and after an instruction.
#[derive(Debug)]
struct AvailableRegisters {
    before: HostRegSet,
    after: HostRegSet,
}

impl<'a, E> RegisterAllocator<'a, E>
where
    E: EmitterAllocRequirements,
{
    /// Create a new register allocator for the target basic block.
    pub fn new(
        translation_unit: &'a TargetBasicBlock,
        emitter_requirements: &'a E,
        host_constraints: RegAllocHostConstraints,
    ) -> Self {
        let available_registers = AvailableRegisters {
            before: host_constraints.usable_registers,
            after: host_constraints.usable_registers,
        };
        RegisterAllocator::<'a, E> {
            target_block: translation_unit,
            emitter_requirements,
            allocations: AllocatorResult::new(translation_unit.instruction_count()),
            loaded_slices: Default::default(),
            host_constraints,
            available_registers,
        }
    }

    pub fn run(mut self) -> JITResult<AllocatorResult> {
        trace_jit!("Running register allocator");
        // Iterate in reverse over all blocks.
        for block in self.target_block.jir_block_ids().rev() {
            self.alloc_block(block)?;
        }
        Ok(self.allocations)
    }

    fn alloc_block(
        &mut self,
        block: JIRBlockId,
    ) -> JITResult {
        for inst in self.target_block.jir_block_inst_ids(block).unwrap().rev() {
            self.alloc_inst(inst)?;
        }
        // TODO: check block inputs and make sure they are allocated here.
        // We are iterating backwards, so we are at the start of a block here,
        // which means we have to load any varnodes that are live in the following instructions.
        Ok(())
    }

    fn reset_available_host_registers(&mut self) {
        self.available_registers.before = self.host_constraints.usable_registers;
        self.available_registers.after = self.host_constraints.usable_registers;
    }

    fn alloc_inst(
        &mut self,
        inst: InstId,
    ) -> JITResult {
        // Start the directives section for this instruction.
        self.allocations
            .load_spill_directives
            .start_inst_directives(inst)?;

        // At the start we set the available registers to all configured registers.
        // We then subsequently remove all registers that are used by clobbers, fixed
        // register operands, or are already allocated.
        self.reset_available_host_registers();

        let clobbers = self.emitter_requirements.clobbers(inst);
        let operands = self.emitter_requirements.operands(inst);
        trace_jit!("Allocating registers for {inst}. It is clobbering {clobbers} and requesting {operands:?}");
        trace_jit!(
            "Available registers at the start before/after:\n{}/{}",
            self.available_registers.before,
            self.available_registers.after
        );

        for operand in operands.fixed() {
            self.reserve_reg_for_fixed_operand(operand)?;
        }
        for clobber in clobbers {
            self.clear_clobber(clobber);
        }

        trace_jit!(
            "Available registers after fixed reg and clobber constraints before/after:\n{}/{}",
            self.available_registers.before,
            self.available_registers.after
        );

        for operand in operands.write() {
            self.alloc_write_operand(operand)?;
        }

        for operand in operands.read() {
            self.alloc_read_operand(operand)?;
        }

        // TODO: We might need to track the number of uses per allocation, such that we can
        //       replace register allocations with memory allocations, in case the value is
        //       only used a certain number of times.

        // Any read operands need to be allocated. No load is necessary here. We simply allocate.
        // Once the slice is deallocated at an earlier instruction, the load will be inserted.

        // Any Write operands need to be allocated, and a write operation needs to be inserted, if the
        // slice is not allocated as write yet.

        // TODO: For now, we most likely just want to spill everything at the end of each block, and assume that
        //       nothing is loaded at the start of each block, for simplicities sake.
        //       In the future, we can maybe put the live out and live in registers into some kind of resolver
        //       That then tries to remap the corresponding registers, instead of flushing them.

        // TODO:
        // if inst.is_branch()
        //     spill all for diverging branches
        //     keep everything loaded for main path (either fallthrough or unconditional jmp)

        self.allocations
            .load_spill_directives
            .finish_inst_directives();
        Ok(())
    }

    fn alloc_write_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult {
        trace_jit!("Allocating write operand {operand:?}");
        Ok(())
    }

    fn alloc_read_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult {
        trace_jit!("Allocating read operand {operand:?}");
        Ok(())
    }

    fn reserve_reg_for_fixed_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult {
        let OperandConstraint::FixedReg(target_reg) = operand.constraint else {
            unreachable!()
        };
        trace_jit!("Reserving register {target_reg} for fixed operand");

        // Check if the register is free, and remove it from the available set.
        match (operand.kind, operand.pos) {
            (OperandKind::Read, OperandPos::Before) => {
                if !self.available_registers.before.contains(target_reg) {
                    todo!("Allocation error")
                }
                self.available_registers.before.remove(target_reg);
            }
            (OperandKind::Write, OperandPos::After) => {
                if !self.available_registers.after.contains(target_reg) {
                    todo!("Allocation error")
                }
                self.available_registers.after.remove(target_reg);
            }
            _ => {
                if !self.available_registers.before.contains(target_reg)
                    || !self.available_registers.after.contains(target_reg)
                {
                    todo!("Allocation error")
                }
                self.available_registers.before.remove(target_reg);
                self.available_registers.after.remove(target_reg);
            }
        }
        // TODO: Is this the correct spill location?
        self.spill_reg(target_reg);
        Ok(())
    }

    #[inline]
    fn clear_clobber(
        &mut self,
        clobber: HostReg,
    ) {
        trace_jit!("Clearing clobber {clobber}");
        if !self.available_registers.before.contains(clobber) || !self.available_registers.after.contains(clobber) {
            todo!(
                "Error: Clobbering already removed register.
                This likely means that a clobber overlaps with a fixed register allocation.
                This must be avoided.
                It could also mean that the register is generally not available, since it is reserved."
            )
        }
        self.spill_reg(clobber);
        self.available_registers.before.remove(clobber);
        self.available_registers.after.remove(clobber);
    }

    /// Spill the provided register at the `after` position of the current instruction.
    #[inline]
    fn spill_reg(
        &mut self,
        reg: HostReg,
    ) {
        // Spilling the register
        if let Some(slice) = self.loaded_slices.take(reg) {
            trace_jit!("Spilling register {reg} to slice {slice}");
            self.allocations
                .load_spill_directives
                .add_load_after(reg, slice);
        }
    }
}
