use crate::compiler::ast::pcode::statement::SASTPCodeStatement::{
    BranchCall,
    BranchReturn,
};
use crate::compiler::ast::pcode::statement::{
    SASTPCodeBranchTarget,
    SASTPCodeStatement,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_ident,
    pcode_label,
};
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::{
    parenthesized,
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter::{
    CloseBracket,
    OpenBracket,
    SemiColon,
};
use crate::compiler::tokens::SleighKeyword::{
    Goto,
    Return,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighToken,
};
use chumsky::prelude::{
    choice,
    none_of,
    via_parser,
};
use chumsky::Parser;
use SASTPCodeStatement::Branch;
use SleighKeyword::Call;

pub fn pcode_conditional_branch_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    let target_recovery = none_of(SleighToken::Control(SemiColon))
        .repeated()
        .to_span()
        .map(SASTPCodeBranchTarget::Invalid)
        .map_with(spanned);

    whitespace_padded(sleigh_keyword(SleighKeyword::If))
        .ignore_then(whitespace_padded(parenthesized(pcode_expression())))
        .then_ignore(sleigh_keyword(SleighKeyword::Goto))
        .then(whitespace_padded(pcode_branch_target().recover_with(via_parser(target_recovery))))
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(|(condition_expression, target)| SASTPCodeStatement::ConditionalBranch {
            condition_expression,
            target,
        })
        .map_with(spanned)
}

pub fn pcode_goto_branch_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    branch_stencil(Goto, Branch)
}

pub fn pcode_call_branch_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    branch_stencil(Call, BranchCall)
}

pub fn pcode_return_branch_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    branch_stencil(Return, BranchReturn)
}

fn branch_stencil<'tokens, F: Fn(Spanned<SASTPCodeBranchTarget>) -> SASTPCodeStatement + Clone>(
    keyword: SleighKeyword,
    target: F,
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sleigh_keyword(keyword))
        .ignore_then(whitespace_padded(pcode_branch_target()))
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(target)
        .map_with(spanned)
}

pub fn pcode_branch_target<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeBranchTarget>, SleighParserExtra<'tokens>> + Clone {
    choice((
        pcode_label().map(SASTPCodeBranchTarget::Label),
        pcode_expression().map(SASTPCodeBranchTarget::Direct),
        pcode_expression()
            .delimited_by(sleigh_control(OpenBracket), sleigh_control(CloseBracket))
            .map(SASTPCodeBranchTarget::Offset),
    ))
    .map_with(spanned)
}
