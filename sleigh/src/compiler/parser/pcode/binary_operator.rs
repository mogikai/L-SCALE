use chumsky::error::Rich;
use chumsky::primitive::choice;
use chumsky::{
    extra,
    Parser,
};

use crate::compiler::ast::pcode::expression::SASTPCodeBinaryOp;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::{
    sleigh_operation,
    whitespace_padded,
};
use crate::compiler::tokens::{
    SleighOperation,
    SleighToken,
};

pub(super) fn binary_operator<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTPCodeBinaryOp>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    whitespace_padded(
        choice((
            sleigh_operation(SleighOperation::Equals)
                .ignore_then(sleigh_operation(SleighOperation::Equals))
                .to(SASTPCodeBinaryOp::IntEquals),
            sleigh_operation(SleighOperation::Or)
                .ignore_then(sleigh_operation(SleighOperation::Or))
                .to(SASTPCodeBinaryOp::BoolOr),
            sleigh_operation(SleighOperation::And)
                .ignore_then(sleigh_operation(SleighOperation::And))
                .to(SASTPCodeBinaryOp::BoolAnd),
            sleigh_operation(SleighOperation::Hat)
                .ignore_then(sleigh_operation(SleighOperation::Hat))
                .to(SASTPCodeBinaryOp::BoolXor),
            sleigh_operation(SleighOperation::LessThan)
                .ignore_then(sleigh_operation(SleighOperation::LessThan))
                .to(SASTPCodeBinaryOp::IntLeft),
            sleigh_operation(SleighOperation::GreaterThan)
                .ignore_then(sleigh_operation(SleighOperation::GreaterThan))
                .to(SASTPCodeBinaryOp::IntRight),
            sleigh_operation(SleighOperation::Or).to(SASTPCodeBinaryOp::IntOr),
            sleigh_operation(SleighOperation::And).to(SASTPCodeBinaryOp::IntAnd),
            sleigh_operation(SleighOperation::Hat).to(SASTPCodeBinaryOp::IntXor),
            sleigh_operation(SleighOperation::Plus).to(SASTPCodeBinaryOp::Add),
            sleigh_operation(SleighOperation::Minus).to(SASTPCodeBinaryOp::Subtract),
        ))
        .map_with(|op, extra| (op, extra.span())),
    )
}
