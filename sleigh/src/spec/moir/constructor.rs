use crate::compiler::SleighCompilerResult;
use crate::spec::jir::instruction::{
    TargetInstruction,
    TargetInstructionBuilder,
    TargetInstructionId,
};
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockId,
};
use crate::spec::jir::local_resolver::JIRResolver;
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
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::{
    LabelId,
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRExport;
use crate::spec::moir::varnode::MOIRLocal;
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecificationError,
};
use crate::{
    jinst,
    new_typed_index_table_id,
};
use itertools::Itertools;
use std::iter::once;
use string_interner::StringInterner;
use string_interner::backend::StringBackend;
use tracing::debug;

#[derive(Debug)]
pub struct MOIRConstructor {
    pub local_scope: MOIRScope,
    pub instructions: Vec<MOIRInstruction>,
    pub display: String,
    /// A constructor may export a value. The export refers to a local in this constructor.
    /// To resolve the value, the scope for this constructor has to be used.
    /// 7.7.2.3. Exports
    ///
    /// The semantic section doesn’t just specify how to generate p-code for a constructor.
    /// Except for those constructors in the root table, this section also associates a
    /// semantic meaning to the table symbol the constructor is part of, allowing the
    /// table to be used as an operand in other tables. The mechanism for making this
    /// association is the export statement. This must be the last statement in the section
    /// and consists of the export keyword followed by the specific symbol to be associated
    /// with the constructor. In general, the constructor will have a sequence of assignment
    /// statements building a final value, and then the varnode containing the value will
    /// be exported. However, anything can be exported.
    ///
    /// `mode: reg++ is addrmode=0x2 & reg { tmp=reg; reg=reg+1; export tmp; }`
    ///
    /// This is an example of a post-increment addressing mode that would be used to build more
    /// complicated instructions. The constructor increments a register reg but stores a copy of
    /// its original value in tmp. The varnode tmp is then exported, associating it with the
    /// table symbol mode. When this constructor is matched, as part of a more complicated instruction,
    /// the symbol mode will represent the original semantic value of reg but with the standard
    /// post-increment side effect.
    ///
    /// The table symbol associated with the constructor becomes a reference to the varnode being exported,
    /// not a copy of the value. If the table symbol is written to, as the left-hand side of an assignment
    /// statement, in some other constructor, the exported varnode is affected. A constant can be exported
    /// if its size as a varnode is given explicitly with the ‘:’ operator.
    ///
    /// It is not legal to put a full expression in an export statement, any expression must appear in an
    /// earlier statement. However, a single ‘&’ operator is allowed as part of the statement, and it behaves
    /// as it would in a normal expression (see Section 7.7.1.6, “Address-of Operator”). It causes the
    /// address of the varnode being modified to be exported as an integer constant.
    ///
    /// The table lists all possible export types so that the compiler can check that if the export
    /// is used on the left hand side of an assignment, the export does not resolve to a const value.
    pub export: Option<MOIRExport>,
}

new_typed_index_table_id! { pub struct ConstructorId; "C"; }

impl MOIRConstructor {
    /// Transform the MOIR code template in the constructor into actual JCode.
    /// This effectively resolves all local variables to either a concrete varnode,
    /// or a constant value. The constructor needs a [JIRResolver] in order
    /// to resolve locals that refer to e.g. fields.
    /// The [JIRResolver] is usually populated by the matcher during matching.
    pub fn generate_jcode(
        &self,
        resolver: JIRResolver,
    ) -> ProcessorSpecResult<TargetInstruction> {
        let mut target_inst = TargetInstructionBuilder::new(resolver.inst_start(), resolver.inst_next());
        for block in self.instructions.chunk_by(|inst, next_inst| {
            !matches!(inst.operation.op_category(), MOIROpCategory::Branch) && next_inst.label.is_none()
        }) {
            if let Some(label) = block.first().expect("Blocks must not be empty").label {
                target_inst.start_labeled_block(label)?
            } else {
                target_inst.start_block()?
            };
            for inst in block {
                self.generate_instruction(&resolver, &mut target_inst, inst)?;
            }
            target_inst.finish_block()?;
        }
        let target_inst = target_inst.finish();
        debug!("{target_inst}");
        Ok(target_inst)
    }

    /// Transform the MOIR code template in the constructor into JCode, but as a subtable.
    /// This means we already have an active block when the function is called, and we do
    /// not need to finish the last block when we return, since the outer constructor
    /// will pick off where we left.
    pub(crate) fn generate_jcode_subtable(
        &self,
        resolver: &JIRResolver,
        inst_builder: &mut TargetInstructionBuilder,
    ) -> ProcessorSpecResult {
        for (i, block) in self
            .instructions
            .chunk_by(|inst, next_inst| {
                !matches!(inst.operation.op_category(), MOIROpCategory::Branch) && next_inst.label.is_none()
            })
            .enumerate()
        {
            if let Some(label) = block.first().expect("Blocks must not be empty").label {
                // If we encounter a labeled instruction, we always start a new block (and finish the old one).
                inst_builder.finish_block()?;
                inst_builder.start_labeled_block(label)?;
            } else if i > 0 {
                // Only start a new block, if it is not the first one. The first block during subtable generation
                // will always be merged into the outer block, if it is not a labeled block.
                inst_builder.finish_block()?;
                inst_builder.start_block()?;
            };
            for inst in block {
                self.generate_instruction(&resolver, inst_builder, inst)?;
            }
            // We only finish blocks when starting new blocks. The last block will remain unfinished, since
            // the parent constructor will pick up generating instructions for that block.
        }
        Ok(())
    }

    fn generate_instruction(
        &self,
        resolver: &JIRResolver,
        target_inst: &mut TargetInstructionBuilder,
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
            MOIROperation::Build(table) => {
                let resolver = resolver.subtable_resolver(*table)?;
                resolver
                    .constructor()
                    .generate_jcode_subtable(resolver, target_inst)?;
                return Ok(());
            }
            MOIROperation::Copy => jinst!({ output? } = { input_a? }),
            MOIROperation::CustomOp { op_id, extra_args } => {
                let args = self.resolve_custom_op_args(resolver, input_a, input_b, extra_args)?;

                if let Ok(output) = output {
                    jinst!(output = (*{ *op_id })(*args))
                } else {
                    jinst!((*{ *op_id })(*args))
                }
            }
            MOIROperation::Nop => return Ok(()),
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
                MOIRGotoType::Regular => jinst!(goto<{ target_inst.resolve_label_to_block(*label) }>),
            },
            MOIROperation::GotoDirectIf => jinst!(if {input_b?} goto {input_a?}),
            MOIROperation::GotoIndirectIf => jinst!(if {input_b?} goto [{input_a?}]),
            MOIROperation::GotoLabelIf(label) => {
                jinst!(if {input_b?} goto <{ target_inst.resolve_label_to_block(*label) }>)
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
        target_inst.push_jir_instruction(inst)
    }

    fn resolve_custom_op_args(
        &self,
        resolver: &JIRResolver,
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
                        .map(|input| resolver.resolve_local(input)),
                )
                .collect::<Result<Vec<_>, _>>();
        }
        todo!("Generate error for invalid inputs to custom op")
    }

    fn resolve_output(
        &self,
        resolver: &JIRResolver,
        output: &Option<MOIRLocalId>,
    ) -> ProcessorSpecResult<Option<JIRSlice>> {
        output
            .map(|out| {
                resolver.resolve_local_id(out).and_then(|o| match o {
                    JIRValue::Slice(slice) => Ok(slice),
                    JIRValue::Const(_) => todo!("Generate error for const not allowed in output position"),
                })
            })
            .transpose()
    }

    fn resolve_input(
        &self,
        resolver: &JIRResolver,
        input: &Option<MOIRLocal>,
    ) -> ProcessorSpecResult<Option<JIRValue>> {
        input.map(|input| resolver.resolve_local(input)).transpose()
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

    /// Returns the size of the export. If the constructor does not export anything, the size is zero.
    pub fn export_size(&self) -> MOIRMaybeSize {
        self.export
            .as_ref()
            .map(|e| e.size(&self.local_scope))
            .unwrap_or(MOIRMaybeSize::Determined(0))
    }

    pub(crate) fn mark_used_locals(&mut self) -> SleighCompilerResult {
        for instruction in &self.instructions {
            if let Some(output) = instruction.output {
                self.local_scope
                    .mark_local_as_used(MOIRLocal::from(output))?;
            }
            if let Some(input_a) = instruction.input_a {
                self.local_scope.mark_local_as_used(input_a)?;
            }
            if let Some(input_b) = instruction.input_b {
                self.local_scope.mark_local_as_used(input_b)?;
            }
            if let MOIROperation::CustomOp { extra_args, .. } = &instruction.operation {
                for arg in extra_args {
                    self.local_scope.mark_local_as_used(*arg)?;
                }
            }
        }
        if let Some(export) = self.export.as_ref() {
            match export {
                MOIRExport::Local(local) => self.local_scope.mark_local_as_used(*local)?,
                MOIRExport::MemReference { address, .. } => self.local_scope.mark_local_as_used(*address)?,
            }
        }
        Ok(())
    }
}
