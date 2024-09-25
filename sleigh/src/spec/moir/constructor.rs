use crate::new_typed_index_table_id;
use crate::spec::ProcessorSpecResult;
use crate::spec::jir::block::JIRBlock;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::local_resolver::JIRLocalResolver;
use crate::spec::moir::code::{
    MOIRInstruction,
    MOIROpCategory,
};
use crate::spec::moir::scope::MOIRScope;
use string_interner::StringInterner;
use string_interner::backend::StringBackend;
use tracing::debug;

#[derive(Debug)]
pub struct MOIRConstructor {
    pub local_scope: MOIRScope,
    pub instructions: Vec<MOIRInstruction>,
    pub display: String,
}

new_typed_index_table_id! { pub struct ConstructorId; "C"; }

impl MOIRConstructor {
    pub fn generate_jcode(
        &self,
        resolver: JIRLocalResolver,
    ) -> ProcessorSpecResult<TargetInstruction> {
        let instruction = TargetInstruction::default();
        let jir_blocks = Vec::<JIRBlock>::new();
        for block in self
            .instructions
            .chunk_by(|inst, _| !matches!(inst.operation.op_category(), MOIROpCategory::Branch))
        {
            let jir_instructions = Vec::<JIRInstruction>::new();
            for instruction in block {
                let output = instruction
                    .output
                    .map(|out| resolver.resolve_local_id(&self.local_scope, out));
                debug!("{instruction}\n");
            }
            debug!("BLOCK END!");
        }
        Ok(instruction)
    }

    pub fn mcode_debug_string(
        &self,
        interner: &StringInterner<StringBackend>,
    ) -> String {
        use std::fmt::Write;
        let mut instructions_str = String::new();
        writeln!(&mut instructions_str, r#"MCode for "{}":"#, self.display).expect("");
        writeln!(&mut instructions_str, "Scope:\n{}", self.local_scope.detailed_scope_display(interner)).expect("");
        writeln!(&mut instructions_str, "Instructions:").expect("");
        for instruction in &self.instructions {
            writeln!(&mut instructions_str, "{}", instruction).expect("");
        }
        instructions_str
    }

    pub fn debug_scope(&self) -> String {
        format!("Scope:\n{:?}", self.local_scope)
    }
}
