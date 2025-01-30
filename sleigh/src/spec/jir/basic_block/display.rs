use crate::spec::jir::basic_block::TargetBasicBlock;
use crate::spec::jir::jcode::JIROperation;
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
        for inst in &self.0.jir_instructions {
            match inst.operation {
                JIROperation::BlockStart(block_id) => writeln!(f, "    +++ {} +++", block_id)?,
                JIROperation::BlockEnd(block_id) => writeln!(f, "    --- {} ---", block_id)?,
                JIROperation::InstructionStart(addr) => writeln!(f, "+++ INST({:?}) +++", addr)?,
                JIROperation::InstructionEnd(addr) => writeln!(f, "--- INST({:?}) ---", addr)?,
                _ => writeln!(f, "        {}", inst)?,
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
