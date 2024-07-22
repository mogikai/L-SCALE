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
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    inline_whitespace_padded,
    sleigh_operation,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighOperation,
    SleighToken,
};

pub(super) fn binary_operator<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeBinaryOp>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(
        choice((
            sleigh_operation(SleighOperation::NotEquals).to(SASTPCodeBinaryOp::IntNotEquals),
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
            sleigh_operation(SleighOperation::Plus).to(SASTPCodeBinaryOp::IntAdd),
            sleigh_operation(SleighOperation::Minus).to(SASTPCodeBinaryOp::IntSubtract),
            sleigh_operation(SleighOperation::Star).to(SASTPCodeBinaryOp::IntMul),
            sleigh_operation(SleighOperation::Division).to(SASTPCodeBinaryOp::IntDiv),
            sleigh_operation(SleighOperation::SDivision).to(SASTPCodeBinaryOp::IntSDiv),
            sleigh_operation(SleighOperation::Remainder).to(SASTPCodeBinaryOp::IntRemainder),
            sleigh_operation(SleighOperation::SRemainder).to(SASTPCodeBinaryOp::IntSRemainder),
            sleigh_operation(SleighOperation::FDivision).to(SASTPCodeBinaryOp::FloatDiv),
            sleigh_operation(SleighOperation::FStar).to(SASTPCodeBinaryOp::FloatMul),
            sleigh_operation(SleighOperation::FPlus).to(SASTPCodeBinaryOp::FloatAdd),
            sleigh_operation(SleighOperation::FMinus).to(SASTPCodeBinaryOp::FloatSubtract),
        ))
        .map_with(spanned),
    )
}
