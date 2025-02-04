use crate::Spanned;
use crate::ast::pcode::expression::SASTPCodeExpression;
use crate::ast::pcode::statement::SASTPCodeStatement;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::pcode::expression::pcode_expression;
use crate::parser::pcode::target_expression::target_expression;
use crate::parser::{
    SleighParserExtra,
    sleigh_control,
    sleigh_keyword,
    sleigh_operation,
    whitespace_padded,
};
use crate::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use chumsky::Parser;
use chumsky::prelude::{
    none_of,
    via_parser,
};

pub fn pcode_assignment_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    // Optional local keyword for assignment operations.
    let local = whitespace_padded(sleigh_keyword(SleighKeyword::Local))
        .or_not()
        .map(|local| local.is_some())
        .map_with(spanned);

    // If parsing an expression fails, try to find the semicolon delimiting the assignment and continue from there.
    let recovery = none_of([SleighToken::Control(SleighControlCharacter::SemiColon)])
        .repeated()
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon));

    local
        .then(whitespace_padded(target_expression()))
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
        .boxed()
}
