mod subpiece;
mod variable;

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
}
