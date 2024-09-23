use crate::new_typed_index_table_id;
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::scope::MOIRScope;
use tracing::debug;

#[derive(Debug)]
pub struct MOIRConstructor {
    pub local_scope: MOIRScope,
    pub instructions: Vec<MOIRInstruction>,
    pub display: String,
}

new_typed_index_table_id! { pub struct ConstructorId; "C"; }

impl MOIRConstructor {
    pub fn debug_instructions(&self) {
        use std::fmt::Write;
        let mut instructions_str = String::new();
        writeln!(&mut instructions_str, r#"JCode for "{}":"#, self.display).expect("");
        writeln!(&mut instructions_str, "Scope:\n{}", self.local_scope).expect("");
        writeln!(&mut instructions_str, "Instructions:").expect("");
        for instruction in &self.instructions {
            writeln!(&mut instructions_str, "{}", instruction).expect("");
        }
        debug!("{instructions_str}");
    }
}
