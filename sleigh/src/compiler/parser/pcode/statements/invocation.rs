use chumsky::Parser;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::pcode_invocation;
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::{
    SleighParserExtra,
    sleigh_control,
};
use crate::compiler::tokens::SleighControlCharacter::SemiColon;

/// A standalone invocation with no output.
/// e.g.: `ident(v0, v1);`
pub fn pcode_standalone_invocation_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    pcode_invocation(pcode_expression())
        .then_ignore(sleigh_control(SemiColon))
        .map(SASTPCodeStatement::Invocation)
        .map_with(spanned)
        .boxed()
}
