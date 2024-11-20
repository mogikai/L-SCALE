use chumsky::Parser;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeUnaryOp,
};
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    SleighParserExtra,
    sleigh_operation,
    whitespace_padded,
};
use crate::compiler::tokens::SleighOperation;
use crate::compiler::tokens::SleighOperation::{
    BitNegate,
    FMinus,
    Minus,
    Not,
};

pub(super) fn unary_operator<'tokens>(
    op: SASTPCodeUnaryOp
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeUnaryOp>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(match op {
        SASTPCodeUnaryOp::IntNegate => sleigh_operation(Minus)
            .to(SASTPCodeUnaryOp::IntNegate)
            .map_with(spanned),
        SASTPCodeUnaryOp::BoolNegate => sleigh_operation(Not)
            .to(SASTPCodeUnaryOp::BoolNegate)
            .map_with(spanned),
        SASTPCodeUnaryOp::BitNegate => sleigh_operation(BitNegate)
            .to(SASTPCodeUnaryOp::BitNegate)
            .map_with(spanned),
        SASTPCodeUnaryOp::FloatNegate => sleigh_operation(FMinus)
            .to(SASTPCodeUnaryOp::FloatNegate)
            .map_with(spanned),
    })
}

pub(super) fn binary_operator<'tokens>(
    op: SASTPCodeBinaryOp
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeBinaryOp>, SleighParserExtra<'tokens>> + Clone {
    use SASTPCodeBinaryOp as BinOp;
    use SleighOperation::*;

    whitespace_padded(
        match op {
            BinOp::IntNotEquals => sleigh_operation(NotEquals).to(BinOp::IntNotEquals).boxed(),
            BinOp::IntSLess => sleigh_operation(SLessThan).to(BinOp::IntSLess).boxed(),
            BinOp::IntSGreater => sleigh_operation(SGreaterThan)
                .to(BinOp::IntSGreater)
                .boxed(),
            BinOp::IntLess => sleigh_operation(LessThan).to(BinOp::IntLess).boxed(),
            BinOp::IntGreater => sleigh_operation(GreaterThan).to(BinOp::IntGreater).boxed(),
            BinOp::FloatLess => sleigh_operation(FLessThan).to(BinOp::FloatLess).boxed(),
            BinOp::FloatGreater => sleigh_operation(FGreaterThan)
                .to(BinOp::FloatGreater)
                .boxed(),
            BinOp::IntOr => sleigh_operation(Or).to(BinOp::IntOr).boxed(),
            BinOp::IntAnd => sleigh_operation(And).to(BinOp::IntAnd).boxed(),
            BinOp::IntXor => sleigh_operation(Hat).to(BinOp::IntXor).boxed(),
            BinOp::IntAdd => sleigh_operation(Plus).to(BinOp::IntAdd).boxed(),
            BinOp::IntSubtract => sleigh_operation(Minus).to(BinOp::IntSubtract).boxed(),
            BinOp::IntMul => sleigh_operation(Star).to(BinOp::IntMul).boxed(),
            BinOp::IntDiv => sleigh_operation(Division).to(BinOp::IntDiv).boxed(),
            BinOp::IntSDiv => sleigh_operation(SDivision).to(BinOp::IntSDiv).boxed(),
            BinOp::IntRemainder => sleigh_operation(Remainder).to(BinOp::IntRemainder).boxed(),
            BinOp::IntSRemainder => sleigh_operation(SRemainder)
                .to(BinOp::IntSRemainder)
                .boxed(),
            BinOp::FloatDiv => sleigh_operation(FDivision).to(BinOp::FloatDiv).boxed(),
            BinOp::FloatMul => sleigh_operation(FStar).to(BinOp::FloatMul).boxed(),
            BinOp::FloatAdd => sleigh_operation(FPlus).to(BinOp::FloatAdd).boxed(),
            BinOp::FloatSubtract => sleigh_operation(FMinus).to(BinOp::FloatSubtract).boxed(),
            BinOp::FloatEquals => sleigh_operation(FEquals).to(BinOp::FloatEquals).boxed(),
            BinOp::FloatNotEquals => sleigh_operation(FNotEquals)
                .to(BinOp::FloatNotEquals)
                .boxed(),
            BinOp::IntEquals => sleigh_operation(Equals)
                .ignore_then(sleigh_operation(Equals))
                .to(BinOp::IntEquals)
                .boxed(),
            BinOp::BoolOr => sleigh_operation(DoubleOr).to(BinOp::BoolOr).boxed(),
            BinOp::BoolAnd => sleigh_operation(DoubleAnd).to(BinOp::BoolAnd).boxed(),
            BinOp::BoolXor => sleigh_operation(Hat)
                .ignore_then(sleigh_operation(Hat))
                .to(BinOp::BoolXor)
                .boxed(),
            BinOp::IntLeft => sleigh_operation(LessThan)
                .ignore_then(sleigh_operation(LessThan))
                .to(BinOp::IntLeft)
                .boxed(),
            BinOp::IntLessEqual => sleigh_operation(LessThan)
                .ignore_then(sleigh_operation(Equals))
                .to(BinOp::IntLessEqual)
                .boxed(),
            BinOp::IntGreaterEqual => sleigh_operation(GreaterThan)
                .ignore_then(sleigh_operation(Equals))
                .to(BinOp::IntGreaterEqual)
                .boxed(),
            BinOp::IntRight => sleigh_operation(GreaterThan)
                .ignore_then(sleigh_operation(GreaterThan))
                .to(BinOp::IntRight)
                .boxed(),
            BinOp::IntSRight => sleigh_operation(SGreaterThan)
                .ignore_then(sleigh_operation(GreaterThan))
                .to(BinOp::IntSRight)
                .boxed(),
            BinOp::IntSLessEqual => sleigh_operation(SLessThan)
                .ignore_then(sleigh_operation(Equals))
                .to(BinOp::IntSLessEqual)
                .boxed(),
            BinOp::IntSGreaterEqual => sleigh_operation(SGreaterThan)
                .ignore_then(sleigh_operation(Equals))
                .to(BinOp::IntSGreaterEqual)
                .boxed(),
            BinOp::FloatLessEqual => sleigh_operation(FLessThan)
                .ignore_then(sleigh_operation(Equals))
                .to(BinOp::FloatLessEqual)
                .boxed(),
            BinOp::FloatGreaterEqual => sleigh_operation(FGreaterThan)
                .ignore_then(sleigh_operation(Equals))
                .to(BinOp::FloatGreaterEqual)
                .boxed(),
        }
        .map_with(spanned),
    )
}
