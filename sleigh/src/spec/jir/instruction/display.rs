use crate::spec::jir::instruction::TargetInstruction;
use std::fmt::{
    Display,
    Formatter,
};

impl Display for TargetInstruction {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(f, "\nTargetInstruction at {:?}:", self.0.start)?;
        writeln!(f, "JCode:")?;
        for (block_id, block) in self.0.jir_blocks.iter_kv() {
            if let Some(block) = block {
                writeln!(f, "{} ---", block_id)?;
                for inst in &self.0.jir_instructions[block.range.clone()] {
                    writeln!(f, "    {}", inst)?;
                }
                writeln!(f, "-->{:?}", block.fallthrough)?;
            } else {
                writeln!(f, "Encountered empty block. This is probably an error.")?;
            }
        }
        writeln!(f, "Next: {:?}", self.0.next)
    }
}
