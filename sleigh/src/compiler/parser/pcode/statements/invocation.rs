use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::{
    parenthesized,
    sast_ident_spanned,
    sleigh_control,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter;
use crate::compiler::tokens::SleighControlCharacter::{
    CloseParen,
    OpenParen,
};

/// A standalone invocation with no output.
/// e.g.: `ident(v0, v1);`
pub fn pcode_standalone_invocation_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    let argument_list = pcode_expression()
        .separated_by(sleigh_control(SleighControlCharacter::Comma))
        .collect::<Vec<_>>();

    whitespace_padded(sast_ident_spanned())
        .then(
            parenthesized(argument_list).or(sleigh_control(OpenParen)
                .ignore_then(sleigh_control(CloseParen))
                .map(|_| Vec::new())),
        )
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(|(name, arguments)| SASTPCodeInvocation { name, arguments })
        .map_with(spanned)
        .map(SASTPCodeStatement::Invocation)
        .map_with(spanned)
        .boxed()
}
