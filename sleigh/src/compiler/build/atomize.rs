mod assignment;
mod expression;
mod invocation;
mod size_inference;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::statement::{
    SASTPCodeBranchTarget,
    SASTPCodeStatement,
};
use crate::compiler::ast::pcode::{
    SASTPCode,
    SASTPCodeLabelledStatement,
};
use crate::compiler::{
    CompilerContext,
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::code::{
    MOIRGotoType,
    MOIRInstruction,
    moir_instruction,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRExport;
use crate::spec::moir::varnode::MOIRReference;

use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::build::trace_build;
use crate::compiler::error::{
    SleighCompilerError,
};
use crate::spec::moir::MOIR_ADDRESS_SIZE;
use crate::spec::moir::address_space::MOIRSpaceType;
use std::fmt::Write;

impl SleighCompiler {
    pub fn atomize_pcode(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        pcode: Spanned<SASTPCode>,
    ) -> SleighCompilerResult<Option<MOIRExport>> {
        // TODO: Create table of locals? Lookup global identifiers?
        let (pcode, _) = pcode.into_tuple();
        scope.make_implicit_builds(instructions)?;
        let export = self.atomize_statements(scope, instructions, pcode.statements)?;
        self.infer_sizes(scope, instructions)?;
        Ok(export.map(Spanned::into_value))
    }

    pub fn atomize_statements(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        statements: Vec<Spanned<SASTPCodeLabelledStatement>>,
    ) -> SleighCompilerResult<Option<Spanned<MOIRExport>>> {
        let mut export: Option<Spanned<MOIRExport>> = None;
        for ((label, statement), _) in statements.into_iter().map(Spanned::into_tuple) {
            let label = if let Some(label) = label {
                // If we have a label, we define it to point to the first instruction of the atomized statement.
                let label_id = instructions.len();
                Some(scope.define_label(label.transmute(), label_id)?)
            } else {
                None
            };
            if let Some(export) = export {
                return Err(span_err!(SleighCompilerError::InstructionAfterExport, export.span().clone(), statement.into_span()));
            }
            let (statement, _) = statement.into_tuple();
            match statement {
                SASTPCodeStatement::Assignment {
                    local,
                    target,
                    expression,
                } => self
                    .atomize_assignment(scope, instructions, local, target, expression),
                SASTPCodeStatement::Invocation(invocation) => {
                    // We ignore the output, since top level invocations do not output anything.
                    // TODO: Maybe it makes sense to update the pcodeop definition here to "no output"?
                    self
                        .atomize_invocation(scope, instructions, None, invocation)
                        .map(|option| ())
                }
                SASTPCodeStatement::Build(table) =>
                    self.atomize_build(scope, instructions, table.transmute()),
                SASTPCodeStatement::Export(expression) => {
                    let span = expression.span().clone();
                    self.atomize_export(scope, instructions, expression)
                        .map(|local| export = Some(Spanned::new(local, span)))
                }
                SASTPCodeStatement::Delayslot(_) => {
                    todo!()
                }
                SASTPCodeStatement::Declaration(ident, size) =>
                    self.atomize_declaration(scope, ident.transmute(), size.map(Spanned::transmute)),
                SASTPCodeStatement::Branch(branch) =>
                    self.atomize_branch(scope, instructions, MOIRGotoType::Regular, branch),
                SASTPCodeStatement::ConditionalBranch {
                    condition_expression,
                    target,
                } => self
                    .atomize_conditional_branch(scope, instructions, condition_expression, target),
                SASTPCodeStatement::BranchCall(target) =>
                    self.atomize_branch(scope, instructions, MOIRGotoType::Call, target),
                SASTPCodeStatement::BranchReturn(target) =>
                    self.atomize_branch(scope, instructions, MOIRGotoType::Return, target),
                SASTPCodeStatement::NOP => {
                    instructions.push(moir_instruction!(nop));
                    Ok(())
                },
                SASTPCodeStatement::Error(_) => {
                    panic!("If there is an error in the AST, then this function should never have been invoked");
                }
            }?;

            if let Some((label, label_id)) = label {
                // If we defined a label for the first instruction, we want to add this information to the first instruction now.
                instructions[label_id].label = Some(label);
            }
        }
        Ok(export)
    }

    fn atomize_export(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRExport> {
        // TODO: Handling dynamic references (e.g. export *:4 tmp) will require keeping track of the
        //       dynamic references somewhere. We need to decide how to do this.
        // Most likely, we will need to atomize the expression, or the expression inside a load expression
        // such that we have a single varnode at the end that represents either the varnode itself, or the
        // address of the desired location.
        // This will then have to be put into some kind of export structure which can be saved as the exported
        // value for a specific constructor. The symbols referring to subtables will remain as-is.
        // Once these values need to be resolved, then we can check the resolve context.
        // Either this will return a normal JIRValue, or we will get an exported value.
        // The exported value will be an enum, with the variants Value and Reference.
        // A Value export will then simply be translated to the varnode.
        // A Reference export will be translated to a load or store instruction. This might encompass
        // allocating a new temporary variable during translation.
        // However, since we need to move the temp allocation to the moir->jir phase anyways, this should
        // be easy.
        let (expression, span) = expression.into_tuple();
        match expression {
            SASTPCodeExpression::Load(deref) => self.atomize_export_dynamic(scope, instructions, deref),
            SASTPCodeExpression::AddressOf(_) => {
                todo!()
            }
            _ => self
                .atomize_expression(scope, instructions, None, Spanned::new(expression, span))
                .map(MOIRExport::Local),
        }
    }

    fn atomize_export_dynamic(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        deref: Spanned<SASTPCodeDeref>,
    ) -> SleighCompilerResult<MOIRExport> {
        let deref = deref.into_value();
        let space = self
            .spec_builder
            .resolve_space(deref.space.map(Spanned::transmute))?;
        let mut address = self.atomize_expression(scope, instructions, None, *deref.expression)?;
        let spc = self
            .spec_builder
            .space_by_symbol_id(space)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate appropriate error report") })?;

        trace_build!("Constraining addr size to {}", spc.address_size);
        scope.constrain_local_size(&mut address, spc.address_size)?;

        let size = deref.size.map(|s| s.into_value()).into();
        trace_build!("Exporting dynamic reference with size {size}");
        Ok(MOIRExport::MemReference { address, space, size })
    }

    fn atomize_build(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        table: Spanned<StringId>,
    ) -> SleighCompilerResult {
        let table = scope
            .local(*table.value())
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error for missing symbol") })?;
        let table = match table.reference {
            MOIRReference::SubTable(table) => table,
            _ => todo!("Generate error for invalid symbol type"),
        };
        scope.mark_explicit_build(table, instructions)?;
        instructions.push(moir_instruction!(build table));
        Ok(())
    }

    fn atomize_declaration(
        &mut self,
        scope: &mut MOIRScope,
        ident: Spanned<StringId>,
        size: Option<Spanned<MOIRSize>>,
    ) -> SleighCompilerResult {
        trace_build!("Atomizing declaration");
        scope
            .allocate_named_temporary(ident, size.map_or(MOIRMaybeSize::Undetermined, |size| size.into_value().into()))
            .map(|_| ())
    }

    fn atomize_conditional_branch(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        condition: Spanned<SASTPCodeExpression>,
        branch: Spanned<SASTPCodeBranchTarget>,
    ) -> SleighCompilerResult {
        let (branch, _) = branch.into_tuple();
        let cond = self.atomize_expression(scope, instructions, None, condition)?;
        match branch {
            SASTPCodeBranchTarget::Label(label) => {
                let target = scope.resolve_label(label.transmute())?;
                instructions.push(moir_instruction!(if (cond) goto <target>))
            }
            SASTPCodeBranchTarget::Indirect(expr) => {
                let target = self.atomize_expression(scope, instructions, None, expr)?;
                instructions.push(moir_instruction!(if (cond) goto [target]))
            }
            SASTPCodeBranchTarget::Direct(expr) => {
                let target = self.atomize_expression(scope, instructions, None, expr)?;
                instructions.push(moir_instruction!(if (cond) goto target))
            }
            SASTPCodeBranchTarget::Invalid(_) => {
                panic!("An invalid branch target should have lead to the compiler quitting earlier.")
            }
        };
        Ok(())
    }

    fn atomize_branch(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        branch_type: MOIRGotoType,
        branch: Spanned<SASTPCodeBranchTarget>,
    ) -> SleighCompilerResult {
        let (branch, _) = branch.into_tuple();
        match branch {
            SASTPCodeBranchTarget::Label(label) => {
                let target = scope.resolve_label(label.transmute())?;
                match branch_type {
                    MOIRGotoType::Call => instructions.push(moir_instruction!(call<target>)),
                    MOIRGotoType::Return => instructions.push(moir_instruction!(return <target>)),
                    MOIRGotoType::Regular => instructions.push(moir_instruction!(goto<target>)),
                }
            }
            SASTPCodeBranchTarget::Indirect(expr) => {
                let target = self.atomize_expression(scope, instructions, None, expr)?;
                match branch_type {
                    MOIRGotoType::Call => instructions.push(moir_instruction!(call[target])),
                    MOIRGotoType::Return => instructions.push(moir_instruction!(return [target])),
                    MOIRGotoType::Regular => instructions.push(moir_instruction!(goto[target])),
                }
            }
            SASTPCodeBranchTarget::Direct(expr) => {
                let target = self.atomize_expression(scope, instructions, None, expr)?;
                match branch_type {
                    MOIRGotoType::Call => instructions.push(moir_instruction!(call target)),
                    MOIRGotoType::Return => instructions.push(moir_instruction!(return target)),
                    MOIRGotoType::Regular => instructions.push(moir_instruction!(goto target)),
                }
            }
            SASTPCodeBranchTarget::Invalid(_) => {
                panic!("An invalid branch target should have lead to the compiler quitting earlier.")
            }
        };
        Ok(())
    }
}
