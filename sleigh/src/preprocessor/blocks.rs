use crate::compiler::ast::Spanned;
use crate::compiler::lexer::SleighTokens;
use crate::spec::StringId;

#[derive(Debug, Clone)]
pub enum BinaryOp {
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone)]
pub enum ComparisonArgument {
    Identifier(StringId),
    Literal(StringId),
}

#[derive(Debug, Clone)]
pub enum EqualityCheck {
    Eq(Spanned<ComparisonArgument>, Spanned<ComparisonArgument>),
    Ne(Spanned<ComparisonArgument>, Spanned<ComparisonArgument>),
}

#[derive(Debug, Clone)]
pub enum Expression {
    BinaryOp(Box<Spanned<Self>>, Spanned<BinaryOp>, Box<Spanned<Self>>),
    EqualityCheck(EqualityCheck),
    Defined(StringId),
}

#[derive(Debug, Clone)]
pub enum PreprocessorBlock {
    Define {
        name: Spanned<StringId>,
        value: Spanned<StringId>,
    },
    Undefine(Spanned<StringId>),
    Include(Spanned<StringId>),
    IfDef {
        ident: Spanned<StringId>,
        if_defined: Option<Spanned<Vec<Spanned<Self>>>>,
        if_undefined: Option<Spanned<Vec<Spanned<Self>>>>,
    },
    If {
        cond: Spanned<Expression>,
        if_true: Spanned<Vec<Spanned<Self>>>,
        if_false: Option<Spanned<Vec<Spanned<Self>>>>,
    },
    PCode(Spanned<SleighTokens>),
    Expansion(Spanned<StringId>),
}
