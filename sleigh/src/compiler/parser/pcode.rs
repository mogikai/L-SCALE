use chumsky::error::Rich;
use chumsky::prelude::{
    choice,
    Recursive,
};
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::binary_operator::binary_operator;
use crate::compiler::parser::{
    parenthesized,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    whitespace_padded,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

mod binary_operator;

pub(super) fn pcode<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTPCode<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Clone {
    let ident = whitespace_padded(sast_ident_spanned());

    let ident_expression = ident.map_with(|ident, extra| (SASTPCodeExpression::Identifier(ident), extra.span()));

    let literal = whitespace_padded(
        sleigh_number().map_with(|num, extra| (SASTPCodeExpression::LiteralNumber((num, extra.span())), extra.span())),
    );

    let declaration = whitespace_padded(sleigh_keyword(SleighKeyword::Local))
        .ignore_then(ident)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|ident, extra| (SASTPCodeStatement::Declaration(ident), extra.span()));

    let local = whitespace_padded(sleigh_keyword(SleighKeyword::Local))
        .or_not()
        .map_with(|local, extra| (local.is_some(), extra.span()));

    let mut expression = Recursive::declare();

    let argument_list = expression
        .clone()
        .separated_by(sleigh_control(SleighControlCharacter::Comma))
        .collect::<Vec<_>>();

    let function_call = whitespace_padded(sast_ident_spanned())
        .then(parenthesized(argument_list))
        .map_with(|(name, arguments), extra| (SASTPCodeExpression::FunctionCall { name, arguments }, extra.span()));

    let atom = function_call
        .clone()
        .or(literal)
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

    let assignment = local
        .then(ident)
        .then_ignore(sleigh_operation(SleighOperation::Equals))
        .then(expression)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|((local, target), expression), extra| {
            (
                SASTPCodeStatement::Assignment {
                    local,
                    target,
                    expression,
                },
                extra.span(),
            )
        });

    let statement = choice((declaration, assignment));

    whitespace_padded(statement)
        .repeated()
        .collect::<Vec<_>>()
        .map_with(|statements, extra| (SASTPCode { statements }, extra.span()))
}
