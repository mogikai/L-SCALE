use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::atomic_expressions::pcode_label;
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::{
    parenthesized,
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighToken,
};
use chumsky::error::Rich;
use chumsky::{
    extra,
    Parser,
};

pub fn pcode_conditional_branch_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sleigh_keyword(SleighKeyword::If))
        .ignore_then(whitespace_padded(parenthesized(pcode_expression())))
        .then_ignore(sleigh_keyword(SleighKeyword::Goto))
        .then(whitespace_padded(pcode_label()))
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|(condition_expression, label), extra| {
            (
                SASTPCodeStatement::ConditionalBranch {
                    condition_expression,
                    label,
                },
                extra.span(),
            )
        })
}
