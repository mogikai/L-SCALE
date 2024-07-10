use chumsky::error::Rich;
use chumsky::prelude::choice;
use chumsky::{
    extra,
    Parser,
};

use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_ident,
    pcode_subpiece,
};
use crate::compiler::tokens::SleighToken;

pub(super) fn target_expression<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTPCodeTargetExpression<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    let ident = pcode_ident();
    let target_expr_ident = ident.map_with(|ident, extra| (SASTPCodeTargetExpression::Variable(ident), extra.span()));
    let subpiece = pcode_subpiece();

    let target_expr_subpiece =
        subpiece.map_with(|subpiece, extra| (SASTPCodeTargetExpression::Subpiece(subpiece), extra.span()));

    choice((target_expr_subpiece, target_expr_ident))
}
