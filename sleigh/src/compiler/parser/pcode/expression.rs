use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeExpression,
};
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_bitrange,
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
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter;
use chumsky::input::MapExtra;
use chumsky::primitive::choice;
use chumsky::recursive::Recursive;
use chumsky::{
    IterParser,
    Parser,
};

pub(super) fn pcode_expression<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeExpression>, SleighParserExtra<'tokens>> + Clone {
    let ident_expression = pcode_ident().map(SASTPCodeExpression::Identifier);

    let bitrange = pcode_bitrange().map(SASTPCodeExpression::Bitrange);

    let number_literal = whitespace_padded(
        sleigh_number()
            .map_with(spanned)
            .map(SASTPCodeExpression::LiteralNumber),
    );

    let expr_subpiece = pcode_subpiece().map(SASTPCodeExpression::Subpiece);

    let mut expression = Recursive::declare();

    let argument_list = expression
        .clone()
        .separated_by(sleigh_control(SleighControlCharacter::Comma))
        .collect::<Vec<_>>();

    let invocation = whitespace_padded(sast_ident_spanned())
        .then(parenthesized(argument_list))
        .map_with(|(name, arguments), extra| {
            SASTPCodeExpression::Invocation((SASTPCodeInvocation { name, arguments }, extra.span()))
        });

    let atom = invocation
        .clone()
        .or(number_literal)
        .or(expr_subpiece)
        .or(bitrange)
        .or(ident_expression)
        .boxed()
        .map_with(spanned)
        .or(parenthesized(expression.clone()));

    let binary_operator = binary_operator();

    let binary_operation = atom
        .clone()
        .foldl_with(binary_operator.then(atom.clone()).repeated(), fold_binary_operation);

    expression.define(whitespace_padded(choice((binary_operation, atom))));
    expression
}

fn fold_binary_operation<'tokens>(
    lhs: Spanned<SASTPCodeExpression>,
    (op, rhs): (Spanned<SASTPCodeBinaryOp>, Spanned<SASTPCodeExpression>),
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTPCodeExpression> {
    (
        SASTPCodeExpression::BinaryOperation {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
        extra.span(),
    )
}
