use chumsky::error::Rich;
use chumsky::primitive::choice;
use chumsky::recursive::Recursive;
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_ident,
    pcode_subpiece,
};
use crate::compiler::parser::pcode::binary_operator::binary_operator;
use crate::compiler::parser::{
    parenthesized,
    sast_ident_spanned,
    sleigh_control,
    sleigh_number,
    whitespace_padded,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};

pub(super) fn pcode_expression<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTPCodeExpression<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Clone {
    let ident_expression =
        pcode_ident().map_with(|ident, extra| (SASTPCodeExpression::Identifier(ident), extra.span()));

    let number_literal = whitespace_padded(
        sleigh_number().map_with(|num, extra| (SASTPCodeExpression::LiteralNumber((num, extra.span())), extra.span())),
    );

    let expr_subpiece =
        pcode_subpiece().map_with(|subpiece, extra| (SASTPCodeExpression::Subpiece(subpiece), extra.span()));

    let mut expression = Recursive::declare();

    let argument_list = expression
        .clone()
        .separated_by(sleigh_control(SleighControlCharacter::Comma))
        .collect::<Vec<_>>();

    let invocation = whitespace_padded(sast_ident_spanned())
        .then(parenthesized(argument_list))
        .map_with(|(name, arguments), extra| {
            (
                SASTPCodeExpression::Invocation((SASTPCodeInvocation { name, arguments }, extra.span())),
                extra.span(),
            )
        });

    let atom = invocation
        .clone()
        .or(number_literal)
        .or(expr_subpiece)
        .or(ident_expression)
        .or(parenthesized(expression.clone()));

    let binary_operator = binary_operator();

    let binary_operation =
        atom.clone()
            .foldl_with(binary_operator.then(atom.clone()).repeated(), |lhs, (op, rhs), extra| {
                (
                    SASTPCodeExpression::BinaryOperation {
                        lhs: Box::new(lhs),
                        op,
                        rhs: Box::new(rhs),
                    },
                    extra.span(),
                )
            });

    expression.define(whitespace_padded(choice((binary_operation, atom))));
    expression
}
