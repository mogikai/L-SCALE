use chumsky::prelude::via_parser;
use chumsky::primitive::none_of;
use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::pcode_label;
use crate::compiler::parser::pcode::statements::pcode_statement;
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
mod expression;
mod operators;
mod statements;
mod target_expression;

pub(super) fn pcode<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCode>, SleighParserExtra<'tokens>> + Clone {
    let recovery = none_of([
        SleighToken::Control(SleighControlCharacter::SemiColon),
        SleighToken::Control(SleighControlCharacter::CloseBrace),
    ])
    .repeated()
    .ignore_then(sleigh_control(SleighControlCharacter::SemiColon));

    // There might be a label at the end of the pcode block without a following statement, effectively
    // allowing a goto statement to skip all instructions in the block. This is implemented internally
    // as a labelled nop operation, which will later just be optimized out.
    let final_label = whitespace_padded(
        pcode_label()
            .map_with(|label, extra| {
                Spanned::new((Some(label), Spanned::new(SASTPCodeStatement::NOP, extra.span())), extra.span())
            })
            .or_not(),
    );

    whitespace_padded(
        pcode_statement().recover_with(via_parser(
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
    .then(final_label)
    // If we have a final label we need to add it to the already parsed statements
    .map(|(mut statements, final_label)| {
        if let Some(final_label) = final_label {
            statements.push(final_label);
        }
        SASTPCode { statements }
    })
    .map_with(spanned)
    .boxed()
}
