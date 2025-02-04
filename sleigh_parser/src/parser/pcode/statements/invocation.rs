use crate::Spanned;
use crate::ast::pcode::statement::SASTPCodeStatement;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::pcode::atomic_expressions::pcode_invocation;
use crate::parser::pcode::expression::pcode_expression;
use crate::parser::{
    SleighParserExtra,
    sleigh_control,
};
use crate::tokens::SleighControlCharacter::SemiColon;
use chumsky::Parser;

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
