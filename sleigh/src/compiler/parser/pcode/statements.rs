use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::SleighParserExtra;
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
use chumsky::Parser;
use chumsky::prelude::choice;

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
    use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
    use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
    use crate::compiler::error::SleighCompilerError;
    use crate::compiler::parser::report_errors;
    use crate::compiler::parser::test::{
        TestResult,
        test_parser,
    };

    use super::*;

    fn test_statement(statement: &str) -> Result<(), SleighCompilerError> {
        test_parser(statement, |cache, context, tokens| {
            report_errors(cache, pcode_statement().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    fn test_statement_output(statement: &str) -> TestResult<Spanned<SASTPCodeStatement>> {
        test_parser(statement, |cache, context, tokens| {
            report_errors(cache, pcode_statement().parse_with_state(tokens.spanned(), context))
        })
        .map(|out| out.1)
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
    fn write_to_deref_parses() -> Result<(), SleighCompilerError> {
        // test_statement("*:10 (Mem + 78) = ST5;")
        test_statement("*x = y;")
    }

    #[test]
    fn load_const_space_literal_to_local_parses() -> Result<(), SleighCompilerError> {
        // test_statement("*:10 (Mem + 78) = ST5;")
        let result = test_statement_output("local x = *[const]:4 1;")?;
        let stmt = result.into_value();
        match stmt {
            SASTPCodeStatement::Assignment {
                local,
                target,
                expression,
            } => {
                assert_eq!(local.into_value(), true);
                match target.into_value() {
                    SASTPCodeTargetExpression::Variable(_) => {}
                    _ => assert!(false, "Invalid target type. Expected Variable."),
                }
                match expression.into_value() {
                    SASTPCodeExpression::Load(deref) => {
                        let deref = deref.into_value();
                        assert!(deref.space.is_some());
                        assert!(deref.size.is_some());
                        match deref.expression.into_value() {
                            SASTPCodeExpression::Literal(_) => {}
                            _ => assert!(false, "Invalid deref expression. Expected Literal"),
                        }
                    }
                    _ => assert!(false, "Invalid rhs type. Expected Load."),
                }
            }
            _ => assert!(false, "Invalid statement parsed. Expected assignment."),
        }
        Ok(())
    }

    #[test]
    fn load_ram_space_ident_parses() -> Result<(), SleighCompilerError> {
        // test_statement("*:10 (Mem + 78) = ST5;")
        let result = test_statement_output("x = *[ram]:4 y;")?;
        let stmt = result.into_value();
        match stmt {
            SASTPCodeStatement::Assignment {
                local,
                target,
                expression,
            } => {
                assert_eq!(local.into_value(), false);
                match target.into_value() {
                    SASTPCodeTargetExpression::Variable(_) => {}
                    _ => assert!(false, "Invalid target type. Expected Variable."),
                }
                match expression.into_value() {
                    SASTPCodeExpression::Load(deref) => {
                        let deref = deref.into_value();
                        assert!(deref.space.is_some());
                        assert!(deref.size.is_some());
                        match deref.expression.into_value() {
                            SASTPCodeExpression::Identifier(_) => {}
                            _ => assert!(false, "Invalid deref expression. Expected Identifier"),
                        }
                    }
                    _ => assert!(false, "Invalid rhs type. Expected Load."),
                }
            }
            _ => assert!(false, "Invalid statement parsed. Expected assignment."),
        }
        Ok(())
    }

    #[test]
    fn load_ram_space_ident_no_size_parses() -> Result<(), SleighCompilerError> {
        // test_statement("*:10 (Mem + 78) = ST5;")
        let result = test_statement_output("x = *[ram] y;")?;
        let stmt = result.into_value();
        match stmt {
            SASTPCodeStatement::Assignment {
                local,
                target,
                expression,
            } => {
                assert_eq!(local.into_value(), false);
                match target.into_value() {
                    SASTPCodeTargetExpression::Variable(_) => {}
                    _ => assert!(false, "Invalid target type. Expected Variable."),
                }
                match expression.into_value() {
                    SASTPCodeExpression::Load(deref) => {
                        let deref = deref.into_value();
                        assert!(deref.space.is_some());
                        assert!(deref.size.is_none());
                        match deref.expression.into_value() {
                            SASTPCodeExpression::Identifier(_) => {}
                            _ => assert!(false, "Invalid deref expression. Expected Identifier"),
                        }
                    }
                    _ => assert!(false, "Invalid rhs type. Expected Load."),
                }
            }
            _ => assert!(false, "Invalid statement parsed. Expected assignment."),
        }
        Ok(())
    }
}
