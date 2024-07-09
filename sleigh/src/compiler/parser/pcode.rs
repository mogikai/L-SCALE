use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};
use chumsky::error::Rich;
use chumsky::prelude::just;
use chumsky::primitive::any;
use chumsky::{
    extra,
    Parser,
};

pub(super) fn pcode<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTPCode<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    any()
        .and_is(just(SleighToken::Control(SleighControlCharacter::CloseBrace)).not())
        .repeated()
        .to_span()
        .map(|span| (SASTPCode { expressions: vec![] }, span))
}
