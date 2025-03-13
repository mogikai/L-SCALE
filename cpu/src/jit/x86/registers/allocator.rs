use crate::jit::api::JITResult;
use crate::jit::trace_jit;
use crate::jit::x86::emitter::EmitterAllocRequirements;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;
use std::ops::Range;

pub struct RegisterAllocator<'a, E> {
    /// The target basic block. It contains the instructions and JirBlocks that the allocator performs allocations for.
    target_block: &'a TargetBasicBlock,

    /// The emitter. It specifies the operand and register requirements for each instruction.
    /// The emitter will later use the allocation result to emit the actual assembly.
    emitter_requirements: &'a E,

    allocations: Allocations,
}

pub struct Allocations {
    allocations: Vec<()>,
    allocation_ranges: Vec<Range<usize>>,
}

impl<'a, E> RegisterAllocator<'a, E>
where
    E: EmitterAllocRequirements,
{
    /// Create a new register allocator for the target basic block.
    pub fn new(
        target_block: &'a TargetBasicBlock,
        emitter_requirements: &'a E,
    ) -> Self {
        RegisterAllocator::<'a, E> {
            target_block,
            emitter_requirements,
            allocations: Allocations {
                allocations: vec![],
                allocation_ranges: vec![],
            },
        }
    }

    pub fn run(self) -> JITResult<Allocations> {
        trace_jit!("Running register allocator");
        for (inst_id, inst) in self.target_block.instructions().enumerate() {
            let clobbers = self.emitter_requirements.clobbers(inst_id);
            let operands = self.emitter_requirements.operands(inst_id);
            trace_jit!("Allocating registers for {inst}. It is clobbering {clobbers:?} and requesting {operands:?}");
        }
        Ok(self.allocations)
    }
}
