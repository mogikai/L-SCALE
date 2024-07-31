use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::pcode::atomic_expressions::pcode_label;
use crate::compiler::parser::pcode::statements::assignment::pcode_assignment_statement;
use crate::compiler::parser::pcode::statements::branching::{
    pcode_call_branch_statement,
    pcode_conditional_branch_statement,
    pcode_goto_branch_statement,
    pcode_return_branch_statement,
};
use crate::compiler::parser::pcode::statements::build::pcode_build_statement;
use crate::compiler::parser::pcode::statements::declaration::pcode_declaration_statement;
use crate::compiler::parser::pcode::statements::export::pcode_export_statement;
use crate::compiler::parser::pcode::statements::invocation::pcode_standalone_invocation_statement;
use crate::compiler::parser::SleighParserExtra;
use chumsky::prelude::choice;
use chumsky::Parser;

pub mod assignment;
pub mod branching;
pub mod build;
pub mod declaration;
mod export;
pub mod invocation;

pub fn pcode_statement<'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens>,
    (Option<Spanned<SASTIdent>>, Spanned<SASTPCodeStatement>),
    SleighParserExtra<'tokens>,
> + Clone {
    let branch_statements = choice((
        pcode_conditional_branch_statement(),
        pcode_goto_branch_statement(),
        pcode_call_branch_statement(),
        pcode_return_branch_statement(),
    ));

    let statement = choice((
        pcode_declaration_statement(),
        pcode_assignment_statement(),
        pcode_standalone_invocation_statement(),
        branch_statements,
        pcode_build_statement(),
        pcode_export_statement(),
    ));

    // statements may have an optional label
    pcode_label().or_not().then(statement).boxed()
}

#[cfg(test)]
mod test {
    use crate::compiler::error::SleighCompilerError;
    use crate::compiler::parser::report_errors;
    use crate::compiler::parser::test::{test_parser, TestResult};

    use super::*;

    fn test_statement(statement: &str) -> Result<(), SleighCompilerError> {
        test_parser(statement, |cache, context, tokens| {
            report_errors(cache, pcode_statement().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn complex_conditional_branch_to_label_parses() -> Result<(), SleighCompilerError> {
        test_statement("if !(zext(Mem) > bnd1_ub) goto <done>;")
    }

    #[test]
    fn conditional_branch_to_label_parses() -> Result<(), SleighCompilerError> {
        test_statement("if(x) goto <y>;")
    }

    #[test]
    fn conditional_branch_to_varnode_parses() -> Result<(), SleighCompilerError> {
        test_statement("if (x[0,32] f< y[0,32]) goto z;")
    }

    #[test]
    fn test_constructor() -> TestResult {
        test_statement("n1: one	is epsilon  [reloc = (((9 & 0x1234) + base) * (!!(1 $or 8) / ((3 $xor displace / ~19))) << (3 | 1) ^ 0xffff) >> (4); ] { export *[const]:1 one; }")
    }

    #[test]
    fn write_to_deref_parses() -> Result<(), SleighCompilerError> {
        // test_statement("*:10 (Mem + 78) = ST5;")
        test_statement("*x = y;")
    }
}
