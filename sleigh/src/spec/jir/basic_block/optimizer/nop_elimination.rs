use crate::spec::{jir::{basic_block::TargetBasicBlockInner, jcode::JIRInstruction, jir_block::JIRBlock}, ProcessorSpecResult};

impl TargetBasicBlockInner {
    pub(super) fn eliminate_nop(&mut self) -> ProcessorSpecResult<u64> {
        let mut elim_count = 0;

        // take ownership to avoid cloning the instructions
        let mut jir_instructions = Vec::with_capacity(self.jir_instructions.len());
        std::mem::swap(&mut jir_instructions, &mut self.jir_instructions);

        for (block_id, block) in self.jir_blocks.items_mut() {
            let new_block_start = self.jir_instructions.len() + 1;

            let Some(block) = block else {
                panic!("This was assigned previously");
            };

            for block_instruction in jir_instructions[(block.range.start - 1)..(block.range.end + 1)].into_iter() {
                if matches!(block_instruction, JIRInstruction::Nop) {
                    elim_count += 1;
                    continue;
                }

                self.jir_instructions.push(block_instruction.clone());
            }

            let new_block_end = self.jir_instructions.len() - 1;
            block.range = new_block_start..new_block_end;
        }

        Ok(elim_count)
    }
}
