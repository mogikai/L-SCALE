use crate::spec::jir::instruction::{
    JIRBlockBuilder,
    TargetInstruction,
    TargetInstructionId,
};
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockId,
};
use crate::spec::jir::local_resolver::JIRLocalResolver;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::code::{
    MOIRGotoType,
    MOIRInstruction,
    MOIROpCategory,
    MOIROperation,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecificationError,
};
use crate::{
    jinst,
    new_typed_index_table_id,
};
use std::iter::once;
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
        let mut target_inst = TargetInstruction::new(resolver.inst_start(), resolver.inst_next());
        let mut previous_block = None;
        for block in self.instructions.chunk_by(|inst, next_inst| {
            !matches!(inst.operation.op_category(), MOIROpCategory::Branch) && next_inst.label.is_none()
        }) {
            let mut jir_block_builder = if let Some(label) = block.first().expect("Blocks must not be empty").label {
                target_inst.block_builder_for_label(label)
            } else {
                target_inst.next_block_builder()
            };
            for inst in block {
                self.generate_instruction(&resolver, &mut jir_block_builder, inst)?;
            }
            let block_id = jir_block_builder.finalize(previous_block);
            // Save the block id of the previous block, so we can attach the next block as successor.
            previous_block = Some(block_id);
            debug!("BLOCK END!");
        }
        debug!("{target_inst}");
        Ok(TargetInstruction::new(0u64.into(), 0u64.into()))
    }

    fn generate_instruction(
        &self,
        resolver: &JIRLocalResolver,
        jir_block_builder: &mut JIRBlockBuilder,
        inst: &MOIRInstruction,
    ) -> ProcessorSpecResult {
        let output = self
            .resolve_output(resolver, &inst.output)?
            .ok_or(ProcessorSpecificationError::MissingOutput);
        let input_a = self
            .resolve_input(resolver, &inst.input_a)?
            .ok_or(ProcessorSpecificationError::MissingInput);
        let input_b = self
            .resolve_input(resolver, &inst.input_b)?
            .ok_or(ProcessorSpecificationError::MissingInput);
        debug!("{inst}\n");
        let inst = match &inst.operation {
            MOIROperation::Build(_) => jinst!(unimpl),
            MOIROperation::Copy => jinst!({ output? } = { input_a? }),
            MOIROperation::CustomOp { op_id, extra_args } => {
                let args = self.resolve_custom_op_args(resolver, input_a, input_b, extra_args)?;

                if let Ok(output) = output {
                    jinst!(output = (*{ *op_id })(*args))
                } else {
                    jinst!((*{ *op_id })(*args))
                }
            }
            MOIROperation::Nop => todo!(),
            MOIROperation::AddressOf => todo!(),
            MOIROperation::PopCount => jinst!({ output? } = popcount({ input_a? })),
            MOIROperation::LZCount => jinst!({ output? } = lzcount({ input_a? })),
            MOIROperation::ZeroExtend => jinst!({ output? } = zext({ input_a? })),
            MOIROperation::SignExtend => jinst!({ output? } = sext({ input_a? })),
            MOIROperation::Carry => jinst!({ output? } = carry({ input_a? }, { input_b? })),
            MOIROperation::SignCarry => jinst!({ output? } = scarry({ input_a? }, { input_b? })),
            MOIROperation::SignBorrow => jinst!({ output? } = sborrow({ input_a? }, { input_b? })),
            MOIROperation::Nan => jinst!({ output? } = nan({ input_a? })),
            MOIROperation::Abs => jinst!({ output? } = abs({ input_a? })),
            MOIROperation::Sqrt => jinst!({ output? } = sqrt({ input_a? })),
            MOIROperation::IntToFloat => jinst!({ output? } = int2float({ input_a? })),
            MOIROperation::FloatToFloat => jinst!({ output? } = float2float({ input_a? })),
            MOIROperation::Trunc => jinst!({ output? } = trunc({ input_a? })),
            MOIROperation::Ceil => jinst!({ output? } = ceil({ input_a? })),
            MOIROperation::Floor => jinst!({ output? } = floor({ input_a? })),
            MOIROperation::Round => jinst!({ output? } = round({ input_a? })),
            MOIROperation::Load(space) => jinst!({ output? } = *[{*space}]{ input_a? }),
            MOIROperation::Store(space) => jinst!(*[{*space}]{ input_a? } = { input_b? }),
            MOIROperation::GotoDirect(ty) => match ty {
                MOIRGotoType::Call => jinst!(call {input_a?}),
                MOIRGotoType::Return => jinst!(return { input_a? }),
                MOIRGotoType::Regular => jinst!(goto {input_a?}),
            },
            MOIROperation::GotoIndirect(ty) => match ty {
                MOIRGotoType::Call => jinst!(call[{ input_a? }]),
                MOIRGotoType::Return => jinst!(return [{ input_a? }]),
                MOIRGotoType::Regular => jinst!(goto[{ input_a? }]),
            },
            MOIROperation::GotoLabel(ty, label) => match ty {
                MOIRGotoType::Call => todo!("JCode internal call currently unsupported"),
                MOIRGotoType::Return => todo!("JCode internal return currently unsupported"),
                MOIRGotoType::Regular => jinst!(goto<{ jir_block_builder.resolve_label_to_block(*label) }>),
            },
            MOIROperation::GotoDirectIf => jinst!(if {input_b?} goto {input_a?}),
            MOIROperation::GotoIndirectIf => jinst!(if {input_b?} goto [{input_a?}]),
            MOIROperation::GotoLabelIf(label) => {
                jinst!(if {input_b?} goto <{ jir_block_builder.resolve_label_to_block(*label) }>)
            }
            MOIROperation::IntAdd => jinst!({ output? } = { input_a? } + { input_b? }),
            MOIROperation::IntSubtract => jinst!({ output? } = { input_a? } - { input_b? }),
            MOIROperation::IntMul => jinst!({ output? } = { input_a? } * { input_b? }),
            MOIROperation::IntDiv => jinst!({ output? } = { input_a? } / { input_b? }),
            MOIROperation::IntSDiv => jinst!({ output? } = { input_a? } s/ { input_b? }),
            MOIROperation::IntRemainder => jinst!({ output? } = { input_a? } % { input_b? }),
            MOIROperation::IntSRemainder => jinst!({ output? } = { input_a? } s% { input_b? }),
            MOIROperation::FloatDiv => jinst!({ output? } = { input_a? } f/ { input_b? }),
            MOIROperation::FloatMul => jinst!({ output? } = { input_a? } f* { input_b? }),
            MOIROperation::FloatAdd => jinst!({ output? } = { input_a? } f+ { input_b? }),
            MOIROperation::FloatSubtract => jinst!({ output? } = { input_a? } f- { input_b? }),
            MOIROperation::BoolOr => jinst!({ output? } = { input_a? } || { input_b? }),
            MOIROperation::BoolAnd => jinst!({ output? } = { input_a? } && { input_b? }),
            MOIROperation::BoolXor => jinst!({ output? } = { input_a? } ^^ { input_b? }),
            MOIROperation::IntLeft => jinst!({ output? } = { input_a? } << { input_b? }),
            MOIROperation::IntRight => jinst!({ output? } = { input_a? } >> { input_b? }),
            MOIROperation::IntSRight => jinst!({ output? } = { input_a? } s>> { input_b? }),
            MOIROperation::IntSLess => jinst!({ output? } = { input_a? } s< { input_b? }),
            MOIROperation::IntSGreater => jinst!({ output? } = { input_a? } s> { input_b? }),
            MOIROperation::IntSLessEqual => jinst!({ output? } = { input_a? } s<= { input_b? }),
            MOIROperation::IntSGreaterEqual => jinst!({ output? } = { input_a? } s>= { input_b? }),
            MOIROperation::IntLess => jinst!({ output? } = { input_a? } < { input_b? }),
            MOIROperation::IntGreater => jinst!({ output? } = { input_a? } > { input_b? }),
            MOIROperation::IntLessEqual => jinst!({ output? } = { input_a? } <= { input_b? }),
            MOIROperation::IntGreaterEqual => jinst!({ output? } = { input_a? } >= { input_b? }),
            MOIROperation::FloatLess => jinst!({ output? } = { input_a? } f< { input_b? }),
            MOIROperation::FloatGreater => jinst!({ output? } = { input_a? } f> { input_b? }),
            MOIROperation::FloatLessEqual => jinst!({ output? } = { input_a? } f<= { input_b? }),
            MOIROperation::FloatGreaterEqual => jinst!({ output? } = { input_a? } f>= { input_b? }),
            MOIROperation::FloatEquals => jinst!({ output? } = { input_a? } f== { input_b? }),
            MOIROperation::FloatNotEquals => jinst!({ output? } = { input_a? } f!= { input_b? }),
            MOIROperation::IntOr => jinst!({ output? } = { input_a? } | { input_b? }),
            MOIROperation::IntAnd => jinst!({ output? } = { input_a? } & { input_b? }),
            MOIROperation::IntXor => jinst!({ output? } = { input_a? } ^ { input_b? }),
            MOIROperation::IntEquals => jinst!({ output? } = { input_a? } == { input_b? }),
            MOIROperation::IntNotEquals => jinst!({ output? } = { input_a? } != { input_b? }),
            MOIROperation::BoolNegate => jinst!({ output? } = !{ input_a? }),
            MOIROperation::BitNegate => jinst!({ output? } = ~{ input_a? }),
            MOIROperation::IntNegate => jinst!({ output? } = -{ input_a? }),
            MOIROperation::FloatNegate => jinst!({ output? } =  fneg{ input_a? }),
        };
        jir_block_builder.push_jir_instruction(inst);
        Ok(())
    }

    fn resolve_custom_op_args(
        &self,
        resolver: &JIRLocalResolver,
        input_a: ProcessorSpecResult<JIRValue>,
        input_b: ProcessorSpecResult<JIRValue>,
        args: &[MOIRLocal],
    ) -> ProcessorSpecResult<Vec<JIRValue>> {
        // TODO: Do this more efficiently?
        if input_a.is_err() && input_b.is_err() {
            return Ok(vec![]);
        }
        if input_a.is_ok() && input_b.is_err() {
            return Ok(vec![input_a?]);
        }
        if input_a.is_ok() && input_b.is_ok() && args.is_empty() {
            return Ok(vec![input_a?, input_b?]);
        }
        if input_a.is_ok() && input_b.is_ok() && !args.is_empty() {
            return once(input_a)
                .chain(once(input_b))
                .chain(
                    args.iter()
                        .copied()
                        .map(|input| resolver.resolve_local(&self.local_scope, input)),
                )
                .collect::<Result<Vec<_>, _>>();
        }
        todo!("Generate error for invalid inputs to custom op")
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
