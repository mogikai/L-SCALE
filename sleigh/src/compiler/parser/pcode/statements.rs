use chumsky::Parser;

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::pcode::atomic_expressions::pcode_label;
use crate::compiler::parser::pcode::statements::assignment::pcode_assignment_statement;
use crate::compiler::parser::pcode::statements::branching::{
    pcode_conditional_branch_statement,
    pcode_goto_branch_statement,
};
use crate::compiler::parser::pcode::statements::build::pcode_build_statement;
use crate::compiler::parser::pcode::statements::declaration::pcode_declaration_statement;
use crate::compiler::parser::pcode::statements::invocation::pcode_standalone_invocation_statement;
use crate::compiler::parser::SleighParserExtra;

pub mod assignment;
pub mod branching;
pub mod build;
pub mod declaration;
pub mod invocation;
pub fn pcode_statement<'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens>,
    (Option<Spanned<SASTIdent>>, Spanned<SASTPCodeStatement>),
    SleighParserExtra<'tokens>,
> + Clone {
    let branch_statements = pcode_conditional_branch_statement().or(pcode_goto_branch_statement());

    let statement = pcode_declaration_statement()
        .or(pcode_assignment_statement())
        .or(pcode_standalone_invocation_statement())
        .or(branch_statements)
        .or(pcode_build_statement())
        .boxed();

    // statements may have an optional label
    pcode_label().or_not().then(statement).boxed()
}

#[cfg(test)]
mod test {
    use crate::compiler::error::SleighCompilerError;
    use crate::compiler::parser::report_errors;
    use crate::compiler::parser::test::test_parser;

    use super::*;

    fn test_statement(statement: &str) -> Result<(), SleighCompilerError> {
        test_parser(statement, |cache, context, tokens| {
            report_errors(cache, pcode_statement().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn conditional_branch_to_label_parses() -> Result<(), SleighCompilerError> {
        test_statement("if(x) goto <y>;")
    }

    #[test]
    fn conditional_branch_to_varnode_parses() -> Result<(), SleighCompilerError> {
        test_statement("if (x[0,32] f< y[0,32]) goto z;")
    }
}
