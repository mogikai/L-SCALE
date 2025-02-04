use crate::Spanned;
use crate::ast::common::{
    SASTIdent,
    SASTSize,
};
use crate::ast::pcode::expression::SASTPCodeExpression;
use crate::ast::pcode::invocation::SASTPCodeInvocation;
use crate::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::lexer::InputSpan;

#[derive(Debug, Clone)]
pub enum SASTPCodeStatement {
    /// Assign the right hand expression to the left hand target expression.
    Assignment {
        local: Spanned<bool>,
        target: Spanned<SASTPCodeTargetExpression>,
        expression: Spanned<SASTPCodeExpression>,
    },
    /// Invoke a function or macro
    Invocation(Spanned<SASTPCodeInvocation>),
    /// Build the provided operand identified by `ident`.
    Build(Spanned<SASTIdent>),
    /// Export the value resulting from the expression
    Export(Spanned<SASTPCodeExpression>),
    Delayslot(Spanned<u64>),
    /// Declare the identifier as a local variable with an optional size.
    Declaration(Spanned<SASTIdent>, Option<Spanned<SASTSize>>),
    Branch(Spanned<SASTPCodeBranchTarget>),
    /// Goto label if `condition_expression` is true. Otherwise, execute the
    /// following instruction
    ConditionalBranch {
        condition_expression: Spanned<SASTPCodeExpression>,
        target: Spanned<SASTPCodeBranchTarget>,
    },
    BranchCall(Spanned<SASTPCodeBranchTarget>),
    BranchReturn(Spanned<SASTPCodeBranchTarget>),
    /// Dummy nop operation, to handle labels at the end of a section.
    NOP,
    /// The statement failed to parse
    Error(InputSpan),
}

#[derive(Debug, Clone)]
pub enum SASTPCodeBranchTarget {
    Label(Spanned<SASTIdent>),
    Indirect(Spanned<SASTPCodeExpression>),
    Direct(Spanned<SASTPCodeExpression>),
    /// The branch target didn't parse correctly
    Invalid(InputSpan),
}
