use chumsky::Parser;
use chumsky::prelude::choice;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::SleighParserExtra;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_bitrange,
    pcode_deref,
    pcode_ident,
    pcode_subpiece,
};
use crate::compiler::parser::pcode::expression::pcode_expression;

pub(super) fn target_expression<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeTargetExpression>, SleighParserExtra<'tokens>> + Clone {
    let ident = pcode_ident().map(SASTPCodeTargetExpression::Variable);
    let subpiece = pcode_subpiece().map(SASTPCodeTargetExpression::Subpiece);
    let bitrange = pcode_bitrange().map(SASTPCodeTargetExpression::BitRange);
    let store = pcode_deref(pcode_expression()).map(SASTPCodeTargetExpression::Store);

    choice((store, subpiece, bitrange, ident))
        .map_with(spanned)
        .boxed()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::compiler::parser::test::{
        TestResult,
        report_errors,
        test_parser,
    };

    fn test_target_expression(expression: &str) -> TestResult {
        test_parser(expression, |cache, context, tokens| {
            report_errors(cache, target_expression().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn ident_target_parses() -> TestResult {
        test_target_expression("x")
    }

    #[test]
    fn subpiece_target_parses() -> TestResult {
        test_target_expression("x:2")?;
        test_target_expression("x : 2")?;
        test_target_expression("x ( 2 )")?;
        test_target_expression("x(2)")
    }

    #[test]
    fn bitrange_target_parses() -> TestResult {
        test_target_expression("x[6,1]")?;
        test_target_expression("x [ 6 , 1 ]")
    }

    #[test]
    fn deref_target_parses() -> TestResult {
        test_target_expression("*x")?;
        test_target_expression("* x")?;
        test_target_expression("*[spc] x")?;
        test_target_expression("* [ spc ] x")?;
        test_target_expression("*[spc]:2 x")?;
        test_target_expression("* [ spc ] : 2 x")?;
        test_target_expression("* :2 x")?;
        test_target_expression("*: 2 x")?;
        test_target_expression("* : 2 x")
    }
}
