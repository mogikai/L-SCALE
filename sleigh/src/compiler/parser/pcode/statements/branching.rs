use chumsky::Parser;

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
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
};

pub fn pcode_conditional_branch_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sleigh_keyword(SleighKeyword::If))
        .ignore_then(whitespace_padded(parenthesized(pcode_expression())))
        .then_ignore(sleigh_keyword(SleighKeyword::Goto))
        .then(whitespace_padded(pcode_label()))
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(|(condition_expression, label)| SASTPCodeStatement::ConditionalBranch {
            condition_expression,
            label,
        })
        .map_with(spanned)
}
