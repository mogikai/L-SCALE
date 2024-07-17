use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::pcode::target_expression::target_expression;
use crate::compiler::parser::{
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
use chumsky::prelude::{
    none_of,
    via_parser,
};
use chumsky::Parser;

pub fn pcode_assignment_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    // Optional local keyword for assignment operations.
    let local = whitespace_padded(sleigh_keyword(SleighKeyword::Local))
        .or_not()
        .map_with(|local, extra| (local.is_some(), extra.span()));

    // If parsing an expression fails, try to find the semicolon delimiting the assignment and continue from there.
    let recovery = none_of([SleighToken::Control(SleighControlCharacter::SemiColon)])
        .repeated()
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon));

    local
        .then(target_expression())
        .then_ignore(sleigh_operation(SleighOperation::Equals))
        .then(
            pcode_expression()
                .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
                .recover_with(via_parser(
                    recovery
                        .to_span()
                        .map(SASTPCodeExpression::Error)
                        .map_with(spanned),
                )),
        )
        .map(|((local, target), expression)| SASTPCodeStatement::Assignment {
            local,
            target,
            expression,
        })
        .map_with(spanned)
}
