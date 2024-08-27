use crate::compiler::ast::common::{
    SASTIdent,
    SASTLiteral,
};
use crate::compiler::ast::pcode::addressof::SASTPCodeAddressOf;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::pcode::expression::SASTPCodeBinaryOp::{
    BoolAnd,
    BoolOr,
    BoolXor,
    FloatEquals,
    FloatGreater,
    FloatGreaterEqual,
    FloatLess,
    FloatLessEqual,
    FloatNotEquals,
    IntEquals,
    IntGreater,
    IntGreaterEqual,
    IntLess,
    IntLessEqual,
    IntNotEquals,
    IntSGreater,
    IntSGreaterEqual,
    IntSLessEqual,
};
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::subpiece::SASTPCodeSubpiece;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;
use std::fmt::{
    Display,
    Formatter,
};
use strum_macros::Display;

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
    Load(Spanned<SASTPCodeDeref>),
    AddressOf(Spanned<SASTPCodeAddressOf>),
    Literal(Spanned<SASTLiteral>),
    Error(InputSpan),
}

impl Display for SASTPCodeExpression {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SASTPCodeExpression::Invocation(_) => write!(f, "invocation"),
            SASTPCodeExpression::BinaryOperation { lhs, op, rhs } => {
                write!(f, "({} {} {})", lhs.value(), op.value(), rhs.value())
            }
            SASTPCodeExpression::UnaryOperation { op, operand } => write!(f, "{} {}", op.value(), operand.value()),
            SASTPCodeExpression::Dereference { space, size } => write!(f, "*{}:{}", space.value(), size.value()),
            SASTPCodeExpression::Subpiece(subpiece) => write!(f, "subpiece"),
            SASTPCodeExpression::Bitrange(_) => write!(f, "bitrange"),
            SASTPCodeExpression::Identifier(ident) => write!(f, "{}", ident.value()),
            SASTPCodeExpression::Load(load) => write!(f, "load"),
            SASTPCodeExpression::AddressOf(_) => write!(f, "address_of"),
            SASTPCodeExpression::Literal(literal) => write!(f, "{}", literal.value()),
            SASTPCodeExpression::Error(_) => write!(f, "error"),
        }
    }
}

#[derive(Debug, Copy, Clone, Display)]
pub enum SASTPCodeBinaryOp {
    /// `v0 + v1`: Addition of v0 and v1 as integers.
    IntAdd,
    /// `v0 - v1`: Subtraction of v1 from v0 as integers.
    IntSubtract,
    /// `v0 * v1`: Integer multiplication of v0 and v1.
    IntMul,
    /// `v0 / v1`: Unsigned division of v0 by v1.
    IntDiv,
    /// `v0 s/ v1`: Signed division of v0 by v1.
    IntSDiv,
    /// `v0 % v1`: Unsigned remainder of v0 modulo v1.
    IntRemainder,
    /// `v0 s% v1`: Signed remainder of v0 modulo v1.
    IntSRemainder,
    /// `v0 f/ v1`: Division of v0 by v1 as floating-point numbers.
    FloatDiv,
    /// `v0 f* v1`: Multiplication of v0 and v1 as floating-point numbers.
    FloatMul,
    /// `v0 f+ v1`: Addition of v0 and v1 as floating-point numbers.
    FloatAdd,
    /// `v0 f- v1`: Subtraction of v1 from v0 as floating-point numbers.
    FloatSubtract,
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
    /// `v0 s>> v1`: Signed (arithmetic) right shift of v0 by b1 bits.
    IntSRight,
    /// `v0 s< v1` or `v1 s> v0`: True if v0 is less than v1 as a signed integer.
    IntSLess,
    /// `v0 s< v1` or `v1 s> v0`: True if v0 is less than v1 as a signed integer.
    IntSGreater,
    /// `v0 s<= v1` or `v1 s>= v0`: True if v0 is less than or equal to v1 as a signed integer.
    IntSLessEqual,
    /// `v0 s<= v1` or `v1 s>= v0`: True if v0 is less than or equal to v1 as a signed integer.
    IntSGreaterEqual,
    /// `v0 < v1` or `v1 > v0`: True if v0 is less than v1 as an unsigned integer.
    IntLess,
    /// `v0 < v1` or `v1 > v0`: True if v0 is less than v1 as an unsigned integer.
    IntGreater,
    /// `v0 <= v1` or `v1 >= v0`: True if v0 is less than or equal to v1 as an unsigned integer.
    IntLessEqual,
    /// `v0 <= v1` or `v1 >= v0`: True if v0 is less than or equal to v1 as an unsigned integer.
    IntGreaterEqual,
    /// `v0 f< v1` or `v1 f> v0`: True if v0 is less than v1 viewed as floating-point numbers.
    FloatLess,
    /// `v0 f< v1` or `v1 f> v0`: True if v0 is less than v1 viewed as floating-point numbers.
    FloatGreater,
    /// `v0 f<= v1` or `v1 f>= v0: True if v0 is less than or equal to v1 as floating-point.
    FloatLessEqual,
    /// `v0 f<= v1` or `v1 f>= v0: True if v0 is less than or equal to v1 as floating-point.
    FloatGreaterEqual,
    /// `v0 f== v1`: True if v0 equals v1 viewed as floating-point numbers.
    FloatEquals,
    /// `v0 f!= v1`: True if v0 does not equal v1 viewed as floating-point numbers.
    FloatNotEquals,
    /// `v0 | v1`: Bitwise Logical Or of v0 with v1.
    IntOr,
    /// `v0 & v1`: Bitwise Logical And of v0 with v1.
    IntAnd,
    /// `v0 ^ v1`: Bitwise Exclusive Or of v0 with v1.
    IntXor,
    /// `v0 == v1`: True if v0 equals v1.
    IntEquals,
    /// `v0 != v1`: True if v0 does not equal v1.
    IntNotEquals,
}

#[derive(Debug, Copy, Clone, Display)]
pub enum SASTPCodeUnaryOp {
    /// `!v0`: Negation of boolean value v0.
    BoolNegate,
    /// `~v0`: Bitwise negation of v0.
    BitNegate,
    /// `-v0`: Twos complement of v0.
    IntNegate,
    /// `f- v0`: Additive inverse of v0 as a floating-point number.
    FloatNegate,
}

impl SASTPCodeBinaryOp {
    pub fn op_category(&self) -> SASTPcodeOpCategory {
        use SASTPCodeBinaryOp::*;
        match self {
            BoolOr | BoolAnd | BoolXor | IntSGreater | IntSLess | IntSLessEqual | IntSGreaterEqual | IntLess
            | IntGreater | IntLessEqual | IntGreaterEqual | FloatLess | FloatGreater | FloatLessEqual
            | FloatGreaterEqual | FloatEquals | FloatNotEquals | IntEquals | IntNotEquals => {
                SASTPcodeOpCategory::Comparison
            }
            IntLeft | IntRight | IntSRight => SASTPcodeOpCategory::Shift,
            _ => SASTPcodeOpCategory::Regular,
        }
    }
}

pub enum SASTPcodeOpCategory {
    /// Comparison operations always produce outputs with one byte in size
    /// and the inputs are only restricted in that they have to have the
    /// same size, independent of the output.
    Comparison,
    /// Shift operations place no restrictions on the size of the shift amount.
    /// The shiftee and output must however have the same sizes.
    Shift,
    /// A regular instruction with no special handling.
    Regular,
}
