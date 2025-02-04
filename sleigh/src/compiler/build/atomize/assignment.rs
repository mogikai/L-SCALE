mod bitrange;
mod store;
mod subpiece;
mod variable;

use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::scope::MOIRScope;
use mogi_sleigh_parser::Spanned;
use mogi_sleigh_parser::ast::pcode::expression::SASTPCodeExpression;
use mogi_sleigh_parser::ast::pcode::target_expression::SASTPCodeTargetExpression;

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
    ) -> InternalCompilerResult {
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
}
