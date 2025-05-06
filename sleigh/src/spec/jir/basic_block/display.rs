use crate::spec::jir::basic_block::TargetBasicBlock;
use crate::spec::jir::jcode::JIRInstruction;
use std::fmt::{
    Display,
    Formatter,
};

impl Display for TargetBasicBlock {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(f, "\nTargetBlock at {:?}:", self.0.start)?;
        writeln!(f, "JCode:")?;
        for (inst_id, inst) in self.0.jir_instructions.iter().enumerate() {
            match inst {
                JIRInstruction::BlockStart(block_id) => writeln!(f, "    +++ [{}]{} +++", inst_id, block_id)?,
                JIRInstruction::BlockEnd(block_id) => writeln!(f, "    --- [{}]{} ---", inst_id, block_id)?,
                JIRInstruction::InstructionStart(addr) => writeln!(f, "+++ INST[{}]({:?}) +++", inst_id, addr)?,
                JIRInstruction::InstructionEnd(addr) => writeln!(f, "--- INST[{}]({:?}) ---", inst_id, addr)?,
                _ => writeln!(f, "        [{}]{}", inst_id, inst)?,
            }
        }
        writeln!(f, "")
        // for (block_id, block) in self.0.jir_blocks.items() {
        //     if let Some(block) = block {
        //         writeln!(f, "{} ---", block_id)?;
        //         for inst in &self.0.jir_instructions[block.range.clone()] {
        //             writeln!(f, "    {}", inst)?;
        //         }
        //         writeln!(f, "-->{:?}", block.fallthrough)?;
        //     } else {
        //         writeln!(f, "Encountered empty block. This is probably an error.")?;
        //     }
        // }
        // writeln!(f, "Next: {:?}", self.0.fallthrough)
    }
}
