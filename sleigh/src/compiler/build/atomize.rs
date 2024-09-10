mod assignment;
mod expression;
mod invocation;
mod size_inference;

use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::statement::{
    SASTPCodeBranchTarget,
    SASTPCodeStatement,
};
use crate::compiler::ast::pcode::{
    SASTPCode,
    SASTPCodeLabelledStatement,
};
use crate::compiler::ast::Spanned;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::code::{
    moir_instruction,
    MOIRGotoType,
    MOIRInstruction,
    MOIROperation,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::{
    InstructionId,
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRExport;
use crate::spec::moir::varnode::MOIRLocal;
use tracing::{
    debug,
    trace,
};

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
        let export = self.atomize_statements(scope, instructions, pcode.statements)?;
        self.infer_sizes(scope, instructions)?;
        debug_instructions(scope, instructions);
        Ok(export.map(|export| match export.into_value() {
            MOIRLocal::Local(local) => MOIRExport::Slice(*scope.resolve_local(local).expect("TODO")),
            MOIRLocal::Const(val) => MOIRExport::Const(val),
        }))
    }

    pub fn atomize_statements(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        statements: Vec<Spanned<SASTPCodeLabelledStatement>>,
    ) -> SleighCompilerResult<Option<Spanned<MOIRLocal>>> {
        let mut export = None;
        for ((label, statement), _) in statements.into_iter().map(Spanned::into_tuple) {
            let label = if let Some(label) = label {
                // If we have a label, we define it to point to the first instruction of the atomized statement.
                let label_id = instructions.len();
                Some(scope.define_label(label.transmute(), label_id)?)
            } else {
                None
            };
            if export.is_some() {
                todo!("Generate error report for instruction after export statement")
            }
            let (statement, _) = statement.into_tuple();
            match statement {
                SASTPCodeStatement::Assignment {
                    local,
                    target,
                    expression,
                } => self.atomize_assignment(scope, instructions, local, target, expression)?,
                SASTPCodeStatement::Invocation(invocation) => {
                    // We ignore the output, since top level invocations do not output anything.
                    // TODO: Maybe it makes sense to update the pcodeop definition here to "no output"?
                    self.atomize_invocation(scope, instructions, None, invocation)?;
                }
                SASTPCodeStatement::Build(table) => instructions.push(moir_instruction!(nop)),
                SASTPCodeStatement::Export(expression) => {
                    let span = expression.span().clone();
                    export = Some(Spanned::new(self.atomize_export(scope, instructions, expression)?, span));
                }
                SASTPCodeStatement::Delayslot(_) => {
                    todo!()
                }
                SASTPCodeStatement::Declaration(ident, size) => {
                    self.atomize_declaration(scope, ident.transmute(), size.map(Spanned::transmute))?
                }
                SASTPCodeStatement::Branch(branch) => {
                    self.atomize_branch(scope, instructions, MOIRGotoType::Regular, branch)?
                }
                SASTPCodeStatement::ConditionalBranch {
                    condition_expression,
                    target,
                } => self.atomize_conditional_branch(scope, instructions, condition_expression, target)?,
                SASTPCodeStatement::BranchCall(target) => {
                    self.atomize_branch(scope, instructions, MOIRGotoType::Call, target)?
                }
                SASTPCodeStatement::BranchReturn(target) => {
                    self.atomize_branch(scope, instructions, MOIRGotoType::Return, target)?
                }
                SASTPCodeStatement::NOP => instructions.push(moir_instruction!(nop)),
                SASTPCodeStatement::Error(_) => {
                    panic!("If there is an error in the AST, then this function should never have been invoked");
                }
            }

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
    ) -> SleighCompilerResult<MOIRLocal> {
        // TODO: Handling dynamic references (e.g. export *:4 tmp) will require keeping track of the
        //       dynamic references somewhere. We need to decide how to do this.
        self.atomize_expression(scope, instructions, None, expression)
    }

    fn atomize_declaration(
        &mut self,
        scope: &mut MOIRScope,
        ident: Spanned<StringId>,
        size: Option<Spanned<MOIRSize>>,
    ) -> SleighCompilerResult {
        trace!("Atomizing declaration");
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

fn debug_instructions(
    scope: &MOIRScope,
    instructions: &[MOIRInstruction],
) {
    let mut instructions_str = String::new();
    writeln!(&mut instructions_str, "The constructor pcode was atomized to the following instructions:").expect("");
    writeln!(&mut instructions_str, "Scope:\n{scope}").expect("");
    writeln!(&mut instructions_str, "Instructions:").expect("");
    for instruction in instructions {
        writeln!(&mut instructions_str, "{}", instruction).expect("");
    }
    debug!("{instructions_str}");
}
