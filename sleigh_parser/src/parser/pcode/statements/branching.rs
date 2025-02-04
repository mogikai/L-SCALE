use chumsky::Parser;
use chumsky::prelude::{
    choice,
    none_of,
    via_parser,
};

use crate::Spanned;
use crate::ast::pcode::statement::SASTPCodeStatement::{
    Branch,
    BranchCall,
    BranchReturn,
};
use crate::ast::pcode::statement::{
    SASTPCodeBranchTarget,
    SASTPCodeStatement,
};
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::pcode::atomic_expressions::pcode_label;
use crate::parser::pcode::expression::pcode_expression;
use crate::parser::{
    SleighParserExtra,
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
};
use crate::tokens::SleighControlCharacter::{
    CloseBracket,
    OpenBracket,
    SemiColon,
};
use crate::tokens::SleighKeyword::{
    Call,
    Goto,
    Return,
};
use crate::tokens::{
    SleighKeyword,
    SleighToken,
};

pub fn pcode_conditional_branch_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    let target_recovery = none_of(SleighToken::Control(SemiColon))
        .repeated()
        .to_span()
        .map(SASTPCodeBranchTarget::Invalid)
        .map_with(spanned);

    whitespace_padded(sleigh_keyword(SleighKeyword::If))
        .ignore_then(whitespace_padded(pcode_expression()))
        .then_ignore(sleigh_keyword(Goto))
        .then(whitespace_padded(pcode_branch_target().recover_with(via_parser(target_recovery))))
        .then_ignore(sleigh_control(SemiColon))
        .map(|(condition_expression, target)| SASTPCodeStatement::ConditionalBranch {
            condition_expression,
            target,
        })
        .map_with(spanned)
        .boxed()
}

pub fn pcode_goto_branch_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    branch_stencil(Goto, Branch)
}

pub fn pcode_call_branch_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    branch_stencil(Call, BranchCall)
}

pub fn pcode_return_branch_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    branch_stencil(Return, BranchReturn)
}

fn branch_stencil<'tokens, F>(
    keyword: SleighKeyword,
    target: F,
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone
where
    F: Fn(Spanned<SASTPCodeBranchTarget>) -> SASTPCodeStatement + Clone + 'tokens,
{
    whitespace_padded(sleigh_keyword(keyword))
        .ignore_then(whitespace_padded(pcode_branch_target()))
        .then_ignore(sleigh_control(SemiColon))
        .map(target)
        .map_with(spanned)
        .boxed()
}

pub fn pcode_branch_target<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeBranchTarget>, SleighParserExtra<'tokens>> + Clone {
    choice((
        pcode_label().map(SASTPCodeBranchTarget::Label),
        pcode_expression().map(SASTPCodeBranchTarget::Direct),
        pcode_expression()
            .delimited_by(sleigh_control(OpenBracket), sleigh_control(CloseBracket))
            .map(SASTPCodeBranchTarget::Indirect),
    ))
    .map_with(spanned)
    .boxed()
}
