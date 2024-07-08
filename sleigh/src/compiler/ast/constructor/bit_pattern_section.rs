use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTBitPatternSection<'src> {
    pub constraint: Spanned<'src, SASTBitPatternConstraint<'src>>,
}

#[derive(Debug)]
pub enum SASTBitPatternConstraint<'src> {
    /// Identifiers may appear on their own in a constraint, linking the local symbol to the global symbol.
    /// See SLEIGH specification on constructors section 7.4.1
    Ident(Spanned<'src, SASTIdent<'src>>),
    /// Constraints can be combined in an arbitrary way using `&`, `|`, and `;`
    BinaryOp(Box<Spanned<'src, Self>>, Spanned<'src, SASTBinaryOp>, Box<Spanned<'src, Self>>),
    /// The only unary operations currently supported are `justify left` and `justify right`
    UnaryOp(Spanned<'src, SASTUnaryOp>, Box<Spanned<'src, Self>>),
    /// An advanced constraint consists of a binary operator where the left hand side is an identifier, and the right
    /// hand side is either an identifier, or a bit pattern (i.e. a number).
    AdvancedConstraint(
        Spanned<'src, SASTIdent<'src>>,
        Spanned<'src, SASTConstraintOp>,
        Spanned<'src, SASTConstraintPattern<'src>>,
    ),
}

#[derive(Debug)]
pub enum SASTBinaryOp {
    And,
    Or,
    Concatenate,
}

#[derive(Debug)]
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
pub enum SASTConstraintPattern<'src> {
    Ident(SASTIdent<'src>),
    BitPattern(u64),
}
