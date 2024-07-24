use chumsky::input::MapExtra;
use chumsky::prelude::{
    none_of,
    recursive,
};
use chumsky::primitive::choice;
use chumsky::recovery::via_parser;
use chumsky::recursive::Recursive;
use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeExpression,
    SASTPCodeUnaryOp,
};
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_bitrange,
    pcode_deref,
    pcode_ident,
    pcode_subpiece,
};
use crate::compiler::parser::pcode::operators::{
    binary_operator,
    unary_operator,
};
use crate::compiler::parser::{
    parenthesized,
    sast_ident_spanned,
    sleigh_control,
    sleigh_number,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter::{
    CloseParen,
    OpenParen,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};

pub(super) fn pcode_expression<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeExpression>, SleighParserExtra<'tokens>> + Clone {
    let ident_expression = pcode_ident().map(SASTPCodeExpression::Identifier);

    let bitrange = pcode_bitrange().map(SASTPCodeExpression::Bitrange);

    let number_literal = whitespace_padded(
        sleigh_number()
            .map_with(spanned)
            .map(SASTPCodeExpression::Literal),
    );

    let expr_subpiece = pcode_subpiece().map(SASTPCodeExpression::Subpiece);

    let mut expression = Recursive::declare();

    let load = pcode_deref(expression.clone()).map(SASTPCodeExpression::Load);

    let argument_list = expression
        .clone()
        .separated_by(sleigh_control(SleighControlCharacter::Comma))
        .collect::<Vec<_>>();

    let invocation = whitespace_padded(sast_ident_spanned())
        .then(parenthesized(argument_list))
        .map(|(name, arguments)| SASTPCodeInvocation { name, arguments })
        .map_with(spanned)
        .map(SASTPCodeExpression::Invocation);

    let atom = whitespace_padded(
        invocation
            .clone()
            .or(expr_subpiece)
            .or(load)
            .or(number_literal)
            .or(bitrange)
            .or(ident_expression)
            .boxed()
            .map_with(spanned)
            .or(parenthesized(expression.clone())),
    );

    let unary_op = unary_operator()
        .repeated()
        .foldr_with(atom.clone(), fold_unary_operation);

    let binary_operator = binary_operator();

    let binary_operation = whitespace_padded(
        unary_op
            .clone()
            .foldl_with(binary_operator.then(unary_op.clone()).repeated(), fold_binary_operation),
    );

    expression.define(choice((binary_operation, atom)));
    expression.boxed()
}

fn fold_unary_operation<'tokens>(
    op: Spanned<SASTPCodeUnaryOp>,
    operand: Spanned<SASTPCodeExpression>,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTPCodeExpression> {
    (
        SASTPCodeExpression::UnaryOperation {
            op,
            operand: Box::new(operand),
        },
        extra.span(),
    )
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

#[cfg(test)]
mod test {
    use chumsky::Parser;

    use crate::compiler::error::SleighCompilerError;
    use crate::compiler::parser::test::TestResult;

    use super::pcode_expression;

    fn test_pcode_expression(expression: &str) -> TestResult {
        use crate::compiler::parser::test::{
            report_errors,
            test_parser,
        };
        test_parser(expression, |cache, context, tokens| {
            report_errors(cache, pcode_expression().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn deref_expression_parses() -> TestResult {
        test_pcode_expression("*[spc]:10 (x + 5)")
    }

    #[test]
    fn func_with_expression_parses() -> TestResult {
        test_pcode_expression("func(x >> (y - z))")
    }

    #[test]
    fn func_with_args_parses() -> TestResult {
        test_pcode_expression("func(a, b)")
    }

    #[test]
    fn complex_expression_parses() -> TestResult {
        test_pcode_expression("(-a * (b - c)) >> d")
    }

    #[test]
    fn subpiece_parses() -> TestResult {
        test_pcode_expression("a:5")?;
        test_pcode_expression("a(5)")?;
        test_pcode_expression("2:4")?;
        test_pcode_expression("2(4)")
    }

    #[test]
    fn operators_parse() -> TestResult {
        test_pcode_expression("!0")?;
        test_pcode_expression("~v0")?;
        test_pcode_expression("-v0")?;
        test_pcode_expression("f- v0")?;
        test_pcode_expression("v0 * v1")?;
        test_pcode_expression("v0 / v1")?;
        test_pcode_expression("v0 s/ v1")?;
        test_pcode_expression("v0 % v1")?;
        test_pcode_expression("v0 s% v1")?;
        test_pcode_expression("v0 f/ v1")?;
        test_pcode_expression("v0 f* v1")?;
        test_pcode_expression("v0 + v1")?;
        test_pcode_expression("v0 - v1")?;
        test_pcode_expression("v0 f+ v1")?;
        test_pcode_expression("v0 f- v1")?;
        test_pcode_expression("v0 << v1")?;
        test_pcode_expression("v0 >> v1")?;
        test_pcode_expression("v0 s>> v1")?;
        test_pcode_expression("v0 s< v1")?;
        test_pcode_expression("v1 s> v0")?;
        test_pcode_expression("v0 s<= v1")?;
        test_pcode_expression("v1 s>= v0")?;
        test_pcode_expression("v0 < v1")?;
        test_pcode_expression("v1 > v0")?;
        test_pcode_expression("v0 <= v1")?;
        test_pcode_expression("v1 >= v0")?;
        test_pcode_expression("v0 f< v1")?;
        test_pcode_expression("v1 f> v0")?;
        test_pcode_expression("v0 f<= v1")?;
        test_pcode_expression("v1 f>= v0")?;
        test_pcode_expression("v0 == v1")?;
        test_pcode_expression("v0 != v1")?;
        test_pcode_expression("v0 f== v1")?;
        test_pcode_expression("v0 f!= v1")?;
        test_pcode_expression("v0 & v1")?;
        test_pcode_expression("v0 ^ v1")?;
        test_pcode_expression("v0 | v1")?;
        test_pcode_expression("v0 ^^ v1")?;
        test_pcode_expression("v0 && v1")?;
        test_pcode_expression("v0 || v1")?;
        test_pcode_expression("zext(v0)")?;
        test_pcode_expression("sext(v0)")?;
        test_pcode_expression("carry(v0,v1)")?;
        test_pcode_expression("scarry(v0,v1)")?;
        test_pcode_expression("sborrow(v0,v1)")?;
        test_pcode_expression("nan(v0)")?;
        test_pcode_expression("abs(v0)")?;
        test_pcode_expression("sqrt(v0)")?;
        test_pcode_expression("int2float(v0)")?;
        test_pcode_expression("float2float(v0)")?;
        test_pcode_expression("trunc(v0)")?;
        test_pcode_expression("ceil(v0)")?;
        test_pcode_expression("floor(v0)")?;
        test_pcode_expression("round(v0)")?;
        test_pcode_expression("cpool(v0, v1, v2)")?;
        test_pcode_expression("newobject(v0)")?;
        test_pcode_expression("ident(v0, v1, v2)")
    }
}
