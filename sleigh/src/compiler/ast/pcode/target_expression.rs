use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::bitrange::SASTBitRange;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::subpiece::SASTPCodeSubpiece;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub enum SASTPCodeTargetExpression<'src> {
    /// Store the right hand side in the variable specified by `identifier`
    Variable(Spanned<'src, SASTIdent<'src>>),
    /// Store the right hand side in the address space `space`, or the default
    /// address space, if no space was provided. `address` is an index into
    /// the specified address space, and `size` is the size of the value to store.
    Store {
        address: Spanned<'src, SASTIdent<'src>>,
        space: Spanned<'src, Option<SASTIdent<'src>>>,
        size: Spanned<'src, Option<u16>>,
    },
    /// Store the right hand side in the range specified. The remaining bits remain unchanged.
    BitRange(Spanned<'src, SASTBitRange<'src>>),
    /// Truncate the variable identified by `identifier` to the supplied size and then assign to the
    /// truncated variable.
    Subpiece(Spanned<'src, SASTPCodeSubpiece<'src>>),
}
