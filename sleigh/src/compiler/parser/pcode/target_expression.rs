use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_bitrange,
    pcode_ident,
    pcode_subpiece,
};
use crate::compiler::parser::SleighParserExtra;
use chumsky::prelude::choice;
use chumsky::Parser;

pub(super) fn target_expression<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeTargetExpression>, SleighParserExtra<'tokens>> + Clone
{
    let ident = pcode_ident().map(SASTPCodeTargetExpression::Variable);
    let subpiece = pcode_subpiece().map(SASTPCodeTargetExpression::Subpiece);
    let bitrange = pcode_bitrange().map(SASTPCodeTargetExpression::BitRange);

    choice((subpiece, bitrange, ident)).map_with(spanned)
}
