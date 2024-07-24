use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::addressof::SASTPCodeAddressOf;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
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
    Load(Spanned<SASTPCodeDeref>),
    AddressOf(Spanned<SASTPCodeAddressOf>),
    Literal(Spanned<u64>),
    Error(InputSpan),
}

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Copy, Clone)]
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
