use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTBitPatternSection {
    pub constraint: Spanned<SASTBitPatternConstraint>,
}

impl SASTBitPatternSection {
    pub fn new(constraint: Spanned<SASTBitPatternConstraint>) -> Self {
        Self { constraint }
    }
}

#[derive(Debug)]
pub enum SASTBitPatternConstraint {
    /// Identifiers may appear on their own in a constraint, linking the local symbol to the global symbol.
    /// See SLEIGH specification on constructors section 7.4.1
    Ident(Spanned<SASTIdent>),
    /// Constraints can be combined in an arbitrary way using `&`, `|`, and `;`
    BinaryOp(Box<Spanned<Self>>, Spanned<SASTBinaryOp>, Box<Spanned<Self>>),
    /// The only unary operations currently supported are `justify left` and `justify right`
    UnaryOp(Spanned<SASTUnaryOp>, Box<Spanned<Self>>),
    /// An advanced constraint consists of a binary operator where the left hand side is an identifier, and the right
    /// hand side is either an identifier, or a bit pattern (i.e. a number).
    AdvancedConstraint(Spanned<SASTIdent>, Spanned<SASTConstraintOp>, Spanned<SASTConstraintPattern>),
    /// Failed to parse the constraint
    Invalid,
}

#[derive(Debug)]
pub enum SASTBinaryOp {
    And,
    Or,
    Concatenate,
}

#[derive(Debug, Copy, Clone)]
pub enum SASTUnaryOp {
    JustifyLeft,
    JustifyRight,
}

#[derive(Debug)]
pub enum SASTConstraintOp {
    Equals,
    NotEquals,
    LessThan,
    GreaterThan,
}

#[derive(Debug)]
pub enum SASTConstraintPattern {
    Ident(Spanned<SASTIdent>),
    BitPattern(Spanned<u64>),
}
