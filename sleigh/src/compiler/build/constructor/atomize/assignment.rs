use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceSelector,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::report::generate_cannot_slice_error;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::{
    moir_instruction,
    MOIRInstruction,
};
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use tracing::trace;

impl SleighCompiler {
    /// Process an assignment statement in the AST. The right hand side of the assignment will
    /// be resolved to multiple atomic instructions that result in a final value which can then
    /// be assigned subsequently. We try to do as much size inference as possible during this
    /// stage, but in some cases it is not possible to infer the sizes, which necessitates the
    /// size inference step at the end of the pcode atomization.
    pub fn atomize_assignment(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        target: Spanned<SASTPCodeTargetExpression>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult {
        let (target, _) = target.into_tuple();
        match target {
            SASTPCodeTargetExpression::Variable(var) => {
                self.atomize_assignment_variable(scope, instructions, local, expression, var)
            }
            SASTPCodeTargetExpression::Store(deref) => {
                self.atomize_assignment_store(scope, instructions, local, expression, deref)
            }
            SASTPCodeTargetExpression::BitRange(bitrange) => {
                self.atomize_assignment_bitrange(scope, instructions, local, expression, bitrange)
            }
            SASTPCodeTargetExpression::Subpiece(subpiece) => {
                self.atomize_assignment_subpiece(scope, instructions, local, expression, subpiece)
            }
        }
    }

    pub fn atomize_assignment_variable(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        var: Spanned<SASTIdent>,
    ) -> SleighCompilerResult<()> {
        // We directly pass the out varnode to the atomize functions to avoid a separate copy operation,
        // since the expressions can directly store their result into this out variable.
        let (lhs, rhs) = if local.into_value() {
            // The variable is not declared yet. We allocate it now.
            let lhs = scope.allocate_named_temporary(var.clone().transmute(), MOIRMaybeSize::Undetermined)?;
            let rhs = self.atomize_expression(scope, instructions, Some(lhs), expression)?;
            (lhs, rhs)
        } else {
            // The variable must already be declared.
            let lhs = scope.resolve_ident(&self.spec_builder, var.transmute())?;
            let rhs = self.atomize_expression(scope, instructions, Some(lhs), expression)?;
            (lhs, rhs)
        };
        if MOIRLocal::from(lhs) != rhs {
            // The atomization could not store into lhs so we need to generate a copy instruction.
            // This can for example happen, if rhs can be folded into a const, in which case
            // we simply copy the const into the lhs here.
            // We can additionally constrain the lhs size to the rhs size if it is determined.
            if scope.size_of_local(lhs).is_undetermined() {
                if let MOIRMaybeSize::Determined(size) = rhs.size(scope) {
                    scope.constrain_local_id_size(lhs, size)?;
                }
            }
            instructions.push(moir_instruction!(lhs = rhs));
        }
        Ok(())
    }

    fn atomize_assignment_store(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        deref: Spanned<SASTPCodeDeref>,
    ) -> SleighCompilerResult<()> {
        if *local {
            todo!("Generate error report for local keyword on store assignment")
        }
        Ok(())
    }

    fn atomize_assignment_bitrange(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        bitrange: Spanned<SASTPCodeBitRange>,
    ) -> SleighCompilerResult<()> {
        // TODO: Actually implement this
        instructions.push(moir_instruction!(nop));
        let (bitrange, _) = bitrange.into_tuple();
        if local.into_value() {
            scope.allocate_named_temporary(bitrange.ident.clone().transmute(), MOIRMaybeSize::Undetermined)?;
        }
        scope.resolve_ident(&self.spec_builder, bitrange.ident.transmute())?;
        Ok(())
    }

    fn atomize_assignment_subpiece(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        subpiece: Spanned<SASTPCodeSubpiece>,
    ) -> SleighCompilerResult<()> {
        // We are dealing with a subpiece assignment. This is similar to a normal assignment.
        // However, we create a new slice and already have the required size, since it is
        // specified in the subpiece operation.
        let (subpiece, _) = subpiece.into_tuple();
        match subpiece.target.into_value() {
            SASTPCodeSubpieceTarget::Ident(ident) => {
                let size = match subpiece.selector.into_value() {
                    SASTPCodeSubpieceSelector::Take(size) => size,
                    SASTPCodeSubpieceSelector::Discard(_) => {
                        todo!("Currently unimplemented")
                    }
                };
                let lhs = if local.into_value() {
                    // We encountered a local definition. Define here and error out if it already exists.
                    scope.allocate_named_temporary(ident.clone().transmute(), size.into())?
                } else {
                    scope.resolve_ident(&self.spec_builder, ident.clone().transmute())?
                };
                // If the subpiece tries to create a slice that is
                // larger than the original, we generate an error.
                if size
                    > scope
                        .size_of_local(lhs)
                        .ok_or_else(|| -> SleighCompilerError {
                            // We cannot slice a value that has an undetermined size
                            todo!("Generate error for undetermined size")
                        })?
                {
                    todo!("Generate error for subpiece being larger than underlying slice.")
                }
                let lhs = scope.truncate_local(lhs, size).map_err(|_| {
                    let (ident, ident_span) = ident.into_tuple();
                    let str = self.spec_builder.resolve_string_id(ident.into());
                    generate_cannot_slice_error(str, ident_span, size)
                })?;
                // The variable is not declared yet. First resolve the rhs and try to determine a size.
                let rhs = self.atomize_expression(scope, instructions, Some(lhs), expression)?;
                if MOIRLocal::from(lhs) != rhs {
                    // The atomization could not directly assign to lhs, so we need to emit a copy.
                    instructions.push(moir_instruction!(lhs = rhs));
                }
            }
            SASTPCodeSubpieceTarget::Literal(_) => {
                todo!("Generate error report for unallowed literals in target expression")
            }
        }
        Ok(())
    }
}
