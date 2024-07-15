use chumsky::error::Rich;
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::{
    parenthesized,
    sast_ident_spanned,
    sleigh_control,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};

/// A standalone invocation with no output.
/// e.g.: `ident(v0, v1);`
pub fn pcode_standalone_invocation_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    let argument_list = pcode_expression()
        .separated_by(sleigh_control(SleighControlCharacter::Comma))
        .collect::<Vec<_>>();

    whitespace_padded(sast_ident_spanned())
        .then(parenthesized(argument_list))
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|(name, arguments), extra| (SASTPCodeInvocation { name, arguments }, extra.span()))
        .map_with(|invocation, extra| (SASTPCodeStatement::Invocation(invocation), extra.span()))
}
