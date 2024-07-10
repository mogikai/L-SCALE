use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub enum SASTPCodeStatement<'src> {
    /// Assign the right hand expression to the left hand target expression.
    Assignment {
        local: Spanned<'src, bool>,
        target: Spanned<'src, SASTPCodeTargetExpression<'src>>,
        expression: Spanned<'src, SASTPCodeExpression<'src>>,
    },
    /// Invoke a function or macro
    Invocation(Spanned<'src, SASTPCodeInvocation<'src>>),
    /// Build the provided operand identified by `ident`.
    Build(Spanned<'src, SASTIdent<'src>>),
    Delayslot(Spanned<'src, u64>),
    /// Declare the identifier as a local variable with an optional size.
    Declaration(Spanned<'src, SASTIdent<'src>>, Option<Spanned<'src, u16>>),
    /// Goto label if `condition_expression` is true. Otherwise, execute the
    /// following instruction
    ConditionalBranch {
        condition_expression: Spanned<'src, SASTPCodeExpression<'src>>,
        label: Spanned<'src, SASTIdent<'src>>,
    },
}
