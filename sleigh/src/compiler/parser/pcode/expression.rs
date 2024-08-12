use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeExpression,
    SASTPCodeUnaryOp,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_address_of,
    pcode_bitrange,
    pcode_deref,
    pcode_ident,
    pcode_invocation,
    pcode_subpiece,
};
use crate::compiler::parser::pcode::operators::{
    binary_operator,
    unary_operator,
};
use crate::compiler::parser::{
    parenthesized,
    sleigh_number,
    whitespace_padded,
    SleighParserExtra,
};
use chumsky::input::MapExtra;
use chumsky::pratt::{
    infix,
    left,
    prefix,
};
use chumsky::primitive::choice;
use chumsky::recursive::Recursive;
use chumsky::{
    IterParser,
    Parser,
};

pub(super) fn pcode_expression<'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeExpression>, SleighParserExtra<'tokens>> + Clone {
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
    let addressof = pcode_address_of(expression.clone()).map(SASTPCodeExpression::AddressOf);

    let invocation = pcode_invocation(expression.clone()).map(SASTPCodeExpression::Invocation);

    let atom = whitespace_padded(
        bitrange
            .or(expr_subpiece)
            .or(load)
            .or(addressof)
            .or(number_literal)
            .or(invocation)
            .or(ident_expression)
            .boxed()
            .map_with(spanned)
            .or(parenthesized(expression.clone())),
    );

    // TODO: Replace with pratt parsing for correct operator precedence.
    // let unary_op = unary_operator()
    //     .repeated()
    //     .foldr_with(atom.clone(), fold_unary_operation);
    //
    // let binary_operator = binary_operator();
    //
    // let binary_operation = whitespace_padded(
    //     unary_op
    //         .clone()
    //         .foldl_with(binary_operator.then(unary_op.clone()).repeated(), fold_binary_operation),
    // );

    use crate::compiler::ast::pcode::expression::{
        SASTPCodeBinaryOp as BOp,
        SASTPCodeUnaryOp as UOp,
    };

    let operation = atom.clone().pratt((
        prefix(34, unary_operator(UOp::IntNegate), fold_unary_operation),
        prefix(33, unary_operator(UOp::BoolNegate), fold_unary_operation),
        prefix(32, unary_operator(UOp::BitNegate), fold_unary_operation),
        prefix(31, unary_operator(UOp::FloatNegate), fold_unary_operation),
        infix(left(30), binary_operator(BOp::IntMul), fold_binary_operation),
        infix(left(29), binary_operator(BOp::IntDiv), fold_binary_operation),
        infix(left(28), binary_operator(BOp::IntSDiv), fold_binary_operation),
        infix(left(27), binary_operator(BOp::IntRemainder), fold_binary_operation),
        infix(left(26), binary_operator(BOp::IntSRemainder), fold_binary_operation),
        infix(left(25), binary_operator(BOp::FloatDiv), fold_binary_operation),
        infix(left(24), binary_operator(BOp::FloatMul), fold_binary_operation),
        infix(left(23), binary_operator(BOp::IntAdd), fold_binary_operation),
        infix(left(22), binary_operator(BOp::IntSubtract), fold_binary_operation),
        infix(left(21), binary_operator(BOp::FloatAdd), fold_binary_operation),
        infix(left(20), binary_operator(BOp::FloatSubtract), fold_binary_operation),
        infix(left(19), binary_operator(BOp::IntLeft), fold_binary_operation),
        infix(left(18), binary_operator(BOp::IntRight), fold_binary_operation),
        infix(left(17), binary_operator(BOp::IntSRight), fold_binary_operation),
        infix(left(16), binary_operator(BOp::IntSLess), fold_binary_operation),
        infix(left(16), binary_operator(BOp::IntSGreater), fold_binary_operation),
        infix(left(15), binary_operator(BOp::IntSLessEqual), fold_binary_operation),
        infix(left(15), binary_operator(BOp::IntSGreaterEqual), fold_binary_operation),
        infix(left(14), binary_operator(BOp::IntLess), fold_binary_operation),
        infix(left(14), binary_operator(BOp::IntGreater), fold_binary_operation),
        infix(left(13), binary_operator(BOp::IntLessEqual), fold_binary_operation),
        infix(left(13), binary_operator(BOp::IntGreaterEqual), fold_binary_operation),
        infix(left(12), binary_operator(BOp::FloatLess), fold_binary_operation),
        infix(left(12), binary_operator(BOp::FloatGreater), fold_binary_operation),
        infix(left(11), binary_operator(BOp::FloatLessEqual), fold_binary_operation),
        infix(left(11), binary_operator(BOp::FloatGreaterEqual), fold_binary_operation),
        infix(left(10), binary_operator(BOp::IntEquals), fold_binary_operation),
        infix(left(9), binary_operator(BOp::IntNotEquals), fold_binary_operation),
        infix(left(8), binary_operator(BOp::FloatEquals), fold_binary_operation),
        infix(left(7), binary_operator(BOp::FloatNotEquals), fold_binary_operation),
        infix(left(6), binary_operator(BOp::IntAnd), fold_binary_operation),
        infix(left(5), binary_operator(BOp::IntXor), fold_binary_operation),
        infix(left(4), binary_operator(BOp::IntOr), fold_binary_operation),
        infix(left(3), binary_operator(BOp::BoolXor), fold_binary_operation),
        infix(left(2), binary_operator(BOp::BoolAnd), fold_binary_operation),
        infix(left(1), binary_operator(BOp::BoolOr), fold_binary_operation),
    ));

    expression.define(choice((operation, atom)));
    expression.boxed()
}

fn fold_unary_operation<'tokens>(
    op: Spanned<SASTPCodeUnaryOp>,
    operand: Spanned<SASTPCodeExpression>,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTPCodeExpression> {
    spanned(
        SASTPCodeExpression::UnaryOperation {
            op,
            operand: Box::new(operand),
        },
        extra,
    )
}

fn fold_binary_operation<'tokens>(
    lhs: Spanned<SASTPCodeExpression>,
    op: Spanned<SASTPCodeBinaryOp>,
    rhs: Spanned<SASTPCodeExpression>,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTPCodeExpression> {
    spanned(
        SASTPCodeExpression::BinaryOperation {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
        extra,
    )
}

#[cfg(test)]
mod test {
    use crate::compiler::ast::pcode::expression::{
        SASTPCodeBinaryOp,
        SASTPCodeExpression,
    };
    use crate::compiler::ast::Spanned;
    use crate::compiler::parser::test::TestResult;
    use chumsky::Parser;

    use super::pcode_expression;

    fn test_pcode_expression(expression: &str) -> TestResult<Spanned<SASTPCodeExpression>> {
        use crate::compiler::parser::test::{
            report_errors,
            test_parser,
        };
        test_parser(expression, |cache, context, tokens| {
            report_errors(cache, pcode_expression().parse_with_state(tokens.spanned(), context))
        })
    }

    #[test]
    fn mul_has_higher_precedence_than_add() -> TestResult {
        let result = test_pcode_expression("a + b * c")?.into_value();
        match result {
            SASTPCodeExpression::BinaryOperation { lhs, op, rhs } => {
                assert!(
                    matches!(op.value(), SASTPCodeBinaryOp::IntAdd),
                    "Expected the outermost operation to be IntAdd"
                )
            }
            _ => panic!("Expected the parser to parse a binary expression."),
        }
        Ok(())
    }

    #[test]
    fn address_of_parses() -> TestResult {
        test_pcode_expression("push88(&:8 inst_next)").map(|_| ())
    }

    #[test]
    fn test() -> TestResult {
        test_pcode_expression("zext( !(xmmTmp1_Da f<= xmmTmp2_Da) ) * 0xFFFFFFFF").map(|_| ())
    }

    #[test]
    fn deref_expression_parses() -> TestResult {
        test_pcode_expression("*[spc]:10 (x + 5)").map(|_| ())
    }

    #[test]
    fn func_with_expression_parses() -> TestResult {
        test_pcode_expression("func(x >> (y - z))").map(|_| ())
    }

    #[test]
    fn func_with_args_parses() -> TestResult {
        test_pcode_expression("func(a, b)").map(|_| ())
    }

    #[test]
    fn func_without_args_parses() -> TestResult {
        test_pcode_expression("func()").map(|_| ())
    }

    #[test]
    fn complex_expression_parses() -> TestResult {
        test_pcode_expression("(-a * (b - c)) >> d").map(|_| ())
    }

    #[test]
    fn subpiece_parses() -> TestResult {
        test_pcode_expression("a:5")?;
        test_pcode_expression("a(5)")?;
        test_pcode_expression("2:4")?;
        test_pcode_expression("2(4)").map(|_| ())
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
        test_pcode_expression("ident(v0, v1, v2)").map(|_| ())
    }
}
