use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::bitrange::SASTBitRange;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::subpiece::SASTPCodeSubpiece;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub enum SASTPCodeExpression<'src> {
    Invocation(Spanned<'src, SASTPCodeInvocation<'src>>),
    BinaryOperation {
        lhs: Box<Spanned<'src, Self>>,
        op: Spanned<'src, SASTPCodeBinaryOp>,
        rhs: Box<Spanned<'src, Self>>,
    },
    UnaryOperation {
        op: Spanned<'src, SASTPCodeUnaryOp>,
        operand: Box<Spanned<'src, Self>>,
    },
    Dereference {
        space: Spanned<'src, SASTIdent<'src>>,
        size: Spanned<'src, u16>,
    },
    Subpiece(Spanned<'src, SASTPCodeSubpiece<'src>>),
    Bitrange(Spanned<'src, SASTBitRange<'src>>),
    Identifier(Spanned<'src, SASTIdent<'src>>),
    LiteralNumber(Spanned<'src, u64>),
}

#[derive(Debug, Copy, Clone)]
pub enum SASTPCodeBinaryOp {
    /// `v0 + v1`: Addition of v0 and v1 as integers.
    Add,
    /// `v0 - v1`: Subtraction of v1 from v0 as integers.
    Subtract,
    /// `v0 || v1`: Logical-Or of booleans v0 and v1.
    BoolOr,
    /// `v0 && v1`: Logical-And of booleans v0 and v1.
    BoolAnd,
    /// `v0 ^^ v1`: Exclusive-Or of booleans v0 and v1.
    BoolXor,
    /// `v0 << v1`: Left shift of v0 by v1 bits.
    IntLeft,
    /// `v0 >> v1`: Unsigned (logical) right shift of v0 by v1 bits.
    IntRight,
    /// `v0 | v1`: Bitwise Logical Or of v0 with v1.
    IntOr,
    /// `v0 & v1`: Bitwise Logical And of v0 with v1.
    IntAnd,
    /// `v0 ^ v1`: Bitwise Exclusive Or of v0 with v1.
    IntXor,
    IntEquals,
}

#[derive(Debug, Copy, Clone)]
pub enum SASTPCodeUnaryOp {
    Not,
}
