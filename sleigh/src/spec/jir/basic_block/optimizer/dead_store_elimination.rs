//! Dead store elimination optimizer.
//! This optimizer removes any stores that are never read.
//! Any varnode that is written to before it has been read is considered dead.
//! Varnodes that have not been read before the end of the block must always be
//! kept, since we don't know if they will be accessed later on.

use crate::spec::ProcessorSpecResult;
use crate::spec::jir::basic_block::TargetBasicBlockInner;

impl TargetBasicBlockInner {
    pub(super) fn eliminate_dead_stores(&mut self) -> ProcessorSpecResult {
        // Instructions that store values that are neither written nor spilled can
        // be determined by a single reverse pass and discarded.

        let mut new_jir_instructions = Vec::new();

        for (block_id, block) in self.jir_blocks.items_mut() {
            let new_block_start = new_jir_instructions.len();

            let Some(block) = block else {
                panic!("This must be Some");
            };

            let dead_stores = self
                .optimizer_ancillary
                .dead_stores
                .get(&block_id)
                .expect("This was populated beforehand");
            let mut dead_store_ptr = dead_stores.len().checked_sub(1);
            for (instr_id, instr) in self.jir_instructions[block.range.clone()]
                .iter()
                .enumerate()
                .map(|(id, inst)| (id + block.range.start, inst))
            {
                if dead_store_ptr.is_some_and(|dsp| instr_id == dead_stores[dsp]) {
                    // dead store found => don't add and shift dead_store_ptr down
                    dead_store_ptr = dead_store_ptr.and_then(|dsp| dsp.checked_sub(1));
                } else {
                    // no dead store => add to new instruction list
                    new_jir_instructions.push(instr.clone());
                }
            }

            // modify block
            let new_block_end = new_jir_instructions.len();
            block.range = new_block_start..new_block_end;
        }

        self.jir_instructions = new_jir_instructions;
        Ok(())
    }
}
