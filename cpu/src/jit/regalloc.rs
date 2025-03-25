mod instruction;
pub mod requirements;
pub mod result;

use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
    HostRegMap,
};
use crate::jit::regalloc::requirements::{
    OperandConstraint,
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

pub struct RegisterAllocator<'a, E> {
    /// The target basic block. It contains the instructions and JirBlocks that the allocator performs allocations for.
    target_block: &'a TargetBasicBlock,

    /// The emitter. It specifies the operand and register requirements for each instruction.
    /// The emitter will later use the allocation result to emit the actual assembly.
    emitter_requirements: &'a E,

    allocations: AllocatorResult,

    /// Maps the [HostReg]
    loaded_slices: HostRegMap<JIRSlice>,
}

impl<'a, E> RegisterAllocator<'a, E>
where
    E: EmitterAllocRequirements,
{
    /// Create a new register allocator for the target basic block.
    pub fn new(
        translation_unit: &'a TargetBasicBlock,
        emitter_requirements: &'a E,
    ) -> Self {
        RegisterAllocator::<'a, E> {
            target_block: translation_unit,
            emitter_requirements,
            allocations: AllocatorResult::default(),
            loaded_slices: Default::default(),
        }
    }

    pub fn run(mut self) -> JITResult<AllocatorResult> {
        trace_jit!("Running register allocator");
        // Iterate in reverse over all blocks.
        for block in self.target_block.jir_block_ids().rev() {
            self.alloc_block(block);
        }
        Ok(self.allocations)
    }

    fn alloc_block(
        &mut self,
        block: JIRBlockId,
    ) {
        for inst in self.target_block.jir_block_inst_ids(block).unwrap().rev() {
            self.alloc_inst(inst);
        }
        // TODO: check block inputs and make sure they are allocated here.
        // We are iterating backwards, so we are at the start of a block here,
        // which means we have to load any varnodes that are live in the following instructions.
    }

    fn alloc_inst(
        &mut self,
        inst: InstId,
    ) {
        let clobbers = self.emitter_requirements.clobbers(inst);
        let operands = self.emitter_requirements.operands(inst);

        for operand in operands.fixed() {
            let OperandConstraint::FixedReg(target_reg) = operand.constraint else {
                unreachable!()
            };
            // TODO: Spill anything in target_reg, if it is not the requested slice.
            // TODO: Process fixed operand
        }
        for clobber in clobbers {
            // TODO: Since we are walking in reverse:
            //       We need to load any allocations appearing in clobbers
            //       and mark the registers as empty for the next (the previous) instruction.

            // Remove from loaded, since this instruction clobbers the register.
            // This means it is free to be allocated to by the next (the previous) instruction.
            if let Some(slice) = self.loaded_slices.take(clobber) {
                // We need to load the slice that is allocated to this register
                // for the previous (the next) instruction.
                self.load_after(inst, clobber, slice)
            }
        }

        // TODO: We might need to track the number of uses per allocation, such that we can
        //       replace register allocations with memory allocations, in case the value is
        //       only used a certain number of times.

        // Any read operands need to be allocated. No load is necessary here. We simply allocate.
        // Once the slice is deallocated at an earlier instruction, the load will be inserted.

        // Any Write operands need to be allocated, and a write operation needs to be inserted, if the
        // slice is not allocated as write yet.

        trace_jit!("Allocating registers for {inst}. It is clobbering {clobbers:?} and requesting {operands:?}");
        // TODO: For now, we most likely just want to spill everything at the end of each block, and assume that
        //       nothing is loaded at the start of each block, for simplicities sake.
        //       In the future, we can maybe put the live out and live in registers into some kind of resolver
        //       That then tries to remap the corresponding registers, instead of flushing them.
        /*

        B1: In(v0, v1)      | Ideally, v0 is loaded once, before B1, and then kept loaded.
            v0 = v0 + v1    |
            v1 = v0 * v0    |
            v2 = v1 + v0    |
            if v2 goto B3   |
        B2: In(v0)          |
            v1 = v0         |
            goto B1         |
        B3: In(v2)          |
            v0 = v2         |

        alloc v0(write)
        alloc v2(read)

         */
    }

    /// Insert a load operation for the [JIRSlice] into the provided [HostReg]
    fn load_after(
        &mut self,
        inst: InstId,
        reg: HostReg,
        slice: JIRSlice,
    ) {
        trace_jit!("Loading {slice} into {reg} after inst: {inst}");
        // TODO: Implement
    }
}
