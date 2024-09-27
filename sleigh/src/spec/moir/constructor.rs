use crate::spec::ProcessorSpecResult;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::JIRBlock;
use crate::spec::jir::local_resolver::JIRLocalResolver;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    MOIROpCategory,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use crate::{
    jcode,
    new_typed_index_table_id,
};
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
        for block in self.instructions.chunk_by(|inst, next_inst| {
            !matches!(inst.operation.op_category(), MOIROpCategory::Branch) && next_inst.label.is_none()
        }) {
            let mut jir_block = JIRBlock::default();
            for instruction in block {
                let output = self.resolve_output(&resolver, &instruction.output)?;
                let input_a = self.resolve_input(&resolver, &instruction.input_a)?;
                let input_b = self.resolve_input(&resolver, &instruction.input_b)?;
                debug!("{instruction}\n");
                if let Some(output) = output {
                    // jir_block.instructions.push(jcode!(output = output));
                }
            }
            debug!("BLOCK END!");
        }
        Ok(instruction)
    }

    fn resolve_output(
        &self,
        resolver: &JIRLocalResolver,
        output: &Option<MOIRLocalId>,
    ) -> ProcessorSpecResult<Option<JIRSlice>> {
        output
            .map(|out| {
                resolver
                    .resolve_local_id(&self.local_scope, out)
                    .and_then(|o| match o {
                        JIRValue::Slice(slice) => Ok(slice),
                        JIRValue::Const(_) => todo!("Generate error for const not allowed in output position"),
                    })
            })
            .transpose()
    }

    fn resolve_input(
        &self,
        resolver: &JIRLocalResolver,
        input: &Option<MOIRLocal>,
    ) -> ProcessorSpecResult<Option<JIRValue>> {
        input
            .map(|input| resolver.resolve_local(&self.local_scope, input))
            .transpose()
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
