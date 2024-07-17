use chumsky::error::Rich;
use chumsky::prelude::choice;
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_ident,
    pcode_u16,
};
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::pcode::target_expression::target_expression;
use crate::compiler::parser::{
    inline_whitespace_padded,
    sleigh_control,
    sleigh_keyword,
    sleigh_operation,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use statements::branching::pcode_conditional_branch_statement;
use statements::invocation::pcode_standalone_invocation_statement;

mod atomic_expressions;
mod binary_operator;
mod expression;
mod statements;
mod target_expression;

pub(super) fn pcode<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCode>, SleighParserExtra<'tokens>> + Clone {
    // Parse a declaration of the form `local some_ident;` or alternatively with a size specifier:
    // `local some_ident_with_size:4;`
    let declaration = whitespace_padded(sleigh_keyword(SleighKeyword::Local))
        .ignore_then(pcode_ident())
        .then(
            sleigh_control(SleighControlCharacter::Colon)
                .ignore_then(pcode_u16())
                .or_not(),
        )
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|(ident, size), extra| (SASTPCodeStatement::Declaration(ident, size), extra.span()));

    // Optional local keyword for assignment operations.
    let local = whitespace_padded(sleigh_keyword(SleighKeyword::Local))
        .or_not()
        .map_with(|local, extra| (local.is_some(), extra.span()));

    let assignment = local
        .then(target_expression())
        .then_ignore(sleigh_operation(SleighOperation::Equals))
        .then(pcode_expression())
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|((local, target), expression), extra| {
            (
                SASTPCodeStatement::Assignment {
                    local,
                    target,
                    expression,
                },
                extra.span(),
            )
        });

    let standalone_invocation = pcode_standalone_invocation_statement();

    let statement = choice((declaration, assignment, standalone_invocation, pcode_conditional_branch_statement()));

    whitespace_padded(statement)
        .repeated()
        .collect::<Vec<_>>()
        .map_with(|statements, extra| (SASTPCode { statements }, extra.span()))
}
