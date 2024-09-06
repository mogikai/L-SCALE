use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::pcode::subpiece::SASTPCodeSubpiece;
use crate::compiler::ast::Spanned;

#[derive(Debug, Clone)]
pub enum SASTPCodeTargetExpression {
    /// Store the right hand side in the variable specified by `identifier`
    Variable(Spanned<SASTIdent>),
    /// Store the right hand side in the address space `space`, or the default
    /// address space, if no space was provided. `address` is an index into
    /// the specified address space, and `size` is the size of the value to store.
    Store(Spanned<SASTPCodeDeref>),
    /// Store the right hand side in the range specified. The remaining bits remain unchanged.
    BitRange(Spanned<SASTPCodeBitRange>),
    /// Truncate the variable identified by `identifier` to the supplied size and then assign to the
    /// truncated variable.
    Subpiece(Spanned<SASTPCodeSubpiece>),
}
