use chumsky::primitive::choice;
use chumsky::Parser;

use crate::compiler::ast::pcode::expression::SASTPCodeBinaryOp;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sleigh_operation,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighOperation;

pub(super) fn binary_operator<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeBinaryOp>, SleighParserExtra<'tokens>> + Clone {
    use SASTPCodeBinaryOp as BinOp;
    use SleighOperation::*;

    let single_token_ops = choice([
        sleigh_operation(NotEquals).to(BinOp::IntNotEquals),
        sleigh_operation(SLessThan).to(BinOp::IntSLess),
        sleigh_operation(SGreaterThan).to(BinOp::IntSGreater),
        sleigh_operation(LessThan).to(BinOp::IntLess),
        sleigh_operation(GreaterThan).to(BinOp::IntGreater),
        sleigh_operation(FLessThan).to(BinOp::FloatLess),
        sleigh_operation(FGreaterThan).to(BinOp::FloatGreater),
        sleigh_operation(Or).to(BinOp::IntOr),
        sleigh_operation(And).to(BinOp::IntAnd),
        sleigh_operation(Hat).to(BinOp::IntXor),
        sleigh_operation(Plus).to(BinOp::IntAdd),
        sleigh_operation(Minus).to(BinOp::IntSubtract),
        sleigh_operation(Star).to(BinOp::IntMul),
        sleigh_operation(Division).to(BinOp::IntDiv),
        sleigh_operation(SDivision).to(BinOp::IntSDiv),
        sleigh_operation(Remainder).to(BinOp::IntRemainder),
        sleigh_operation(SRemainder).to(BinOp::IntSRemainder),
        sleigh_operation(FDivision).to(BinOp::FloatDiv),
        sleigh_operation(FStar).to(BinOp::FloatMul),
        sleigh_operation(FPlus).to(BinOp::FloatAdd),
        sleigh_operation(FMinus).to(BinOp::FloatSubtract),
    ]);
    let multi_token_ops = choice([
        sleigh_operation(Equals)
            .ignore_then(sleigh_operation(Equals))
            .to(BinOp::IntEquals),
        sleigh_operation(Or)
            .ignore_then(sleigh_operation(Or))
            .to(BinOp::BoolOr),
        sleigh_operation(And)
            .ignore_then(sleigh_operation(And))
            .to(BinOp::BoolAnd),
        sleigh_operation(Hat)
            .ignore_then(sleigh_operation(Hat))
            .to(BinOp::BoolXor),
        sleigh_operation(LessThan)
            .ignore_then(sleigh_operation(LessThan))
            .to(BinOp::IntLeft),
        sleigh_operation(GreaterThan)
            .ignore_then(sleigh_operation(GreaterThan))
            .to(BinOp::IntRight),
        sleigh_operation(SGreaterThan)
            .ignore_then(sleigh_operation(GreaterThan))
            .to(BinOp::IntSRight),
        sleigh_operation(SLessThan)
            .ignore_then(sleigh_operation(Equals))
            .to(BinOp::IntSLessEqual),
        sleigh_operation(SGreaterThan)
            .ignore_then(sleigh_operation(Equals))
            .to(BinOp::IntSGreaterEqual),
        sleigh_operation(FLessThan)
            .ignore_then(sleigh_operation(Equals))
            .to(BinOp::FloatLessEqual),
        sleigh_operation(FGreaterThan)
            .ignore_then(sleigh_operation(Equals))
            .to(BinOp::FloatGreaterEqual),
    ]);
    whitespace_padded(single_token_ops.or(multi_token_ops).map_with(spanned))
}
