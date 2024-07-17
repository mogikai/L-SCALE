use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::subpiece::SASTPCodeSubpiece;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;

#[derive(Debug)]
pub enum SASTPCodeExpression {
    Invocation(Spanned<SASTPCodeInvocation>),
    BinaryOperation {
        lhs: Box<Spanned<Self>>,
        op: Spanned<SASTPCodeBinaryOp>,
        rhs: Box<Spanned<Self>>,
    },
    UnaryOperation {
        op: Spanned<SASTPCodeUnaryOp>,
        operand: Box<Spanned<Self>>,
    },
    Dereference {
        space: Spanned<SASTIdent>,
        size: Spanned<u16>,
    },
    Subpiece(Spanned<SASTPCodeSubpiece>),
    Bitrange(Spanned<SASTPCodeBitRange>),
    Identifier(Spanned<SASTIdent>),
    LiteralNumber(Spanned<u64>),
    Error(InputSpan),
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
