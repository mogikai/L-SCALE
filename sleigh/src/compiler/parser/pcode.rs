use chumsky::prelude::via_parser;
use chumsky::primitive::none_of;
use chumsky::{
    IterParser,
    Parser,
};

use statements::branching::pcode_conditional_branch_statement;
use statements::invocation::pcode_standalone_invocation_statement;

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::pcode_label;
use crate::compiler::parser::pcode::statements::assignment::pcode_assignment_statement;
use crate::compiler::parser::pcode::statements::branching::pcode_goto_branch_statement;
use crate::compiler::parser::pcode::statements::declaration::pcode_declaration_statement;
use crate::compiler::parser::{
    sleigh_control,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};

mod atomic_expressions;
mod binary_operator;
mod expression;
mod statements;
mod target_expression;

pub(super) fn pcode<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCode>, SleighParserExtra<'tokens>> + Clone {
    let branch_statements = pcode_conditional_branch_statement().or(pcode_goto_branch_statement());

    let statement = pcode_declaration_statement()
        .or(pcode_assignment_statement())
        .or(pcode_standalone_invocation_statement())
        .or(branch_statements)
        .boxed();

    // statements may have an optional label
    let statement = pcode_label().or_not().then(statement);

    let recovery = none_of([
        SleighToken::Control(SleighControlCharacter::SemiColon),
        SleighToken::Control(SleighControlCharacter::CloseBrace),
    ])
    .repeated()
    .ignore_then(sleigh_control(SleighControlCharacter::SemiColon));

    whitespace_padded(
        statement.recover_with(via_parser(
            recovery
                .to_span()
                .map(SASTPCodeStatement::Error)
                .map_with(spanned)
                .map(|x| (None, x)),
        )),
    )
    .map_with(spanned)
    .repeated()
    .collect::<Vec<_>>()
    .map(|statements| SASTPCode { statements })
    .map_with(spanned)
    .boxed()
}
