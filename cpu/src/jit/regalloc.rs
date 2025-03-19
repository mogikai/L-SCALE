mod instruction;
pub mod requirements;
pub mod result;

use crate::jit::api::JITResult;
use crate::jit::trace_jit;
use mogi_sleigh::spec::jir::basic_block::{
    InstId,
    TargetBasicBlock,
};
use mogi_sleigh::spec::jir::jir_block::JIRBlockId;
use requirements::EmitterAllocRequirements;
use result::AllocatorResult;
use std::fmt::Display;
use std::ops::Range;

pub struct RegisterAllocator<'a, E> {
    /// The target basic block. It contains the instructions and JirBlocks that the allocator performs allocations for.
    target_block: &'a TargetBasicBlock,

    /// The emitter. It specifies the operand and register requirements for each instruction.
    /// The emitter will later use the allocation result to emit the actual assembly.
    emitter_requirements: &'a E,

    allocations: AllocatorResult,
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
}
