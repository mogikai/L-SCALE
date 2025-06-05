use crate::spec::ProcessorSpecResult;
use crate::spec::jir::basic_block::TargetBasicBlockInner;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockFallthrough,
};

impl TargetBasicBlockInner {
    pub(super) fn eliminate_nop(&mut self) -> ProcessorSpecResult<u64> {
        let mut elim_count = 0;

        // take ownership to avoid cloning the instructions
        let mut jir_instructions = Vec::with_capacity(self.jir_instructions.len());

        let mut current_block_id = self.jir_blocks.keys().next();
        while let Some(block_id) = current_block_id {
            let block_range = self.jir_blocks[block_id]
                .as_ref()
                .expect("This block must exist")
                .range
                .clone();

            jir_instructions.push(JIRInstruction::BlockStart(block_id));

            let new_block_start = jir_instructions.len();

            for block_instruction in self.jir_instructions[block_range].into_iter() {
                if matches!(block_instruction, JIRInstruction::Nop) {
                    elim_count += 1;
                    continue;
                }

                jir_instructions.push(block_instruction.clone());
            }

            let new_block_end = jir_instructions.len();
            let new_range = new_block_start..new_block_end;
            self.jir_blocks[block_id]
                .as_mut()
                .expect("This block must exist")
                .range = new_range;

            jir_instructions.push(JIRInstruction::BlockEnd(block_id));

            current_block_id = match self.jir_blocks[block_id].as_ref().unwrap().fallthrough {
                JIRBlockFallthrough::NextInst => None,
                JIRBlockFallthrough::Block(jirblock_id) => Some(jirblock_id),
            };
        }

        self.jir_instructions = jir_instructions;

        Ok(elim_count)
    }
}
