use crate::compiler::ast::constructor::semantic_section::SASTSemanticSection;
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
use chumsky::prelude::{
    any,
    just,
};
use chumsky::{
    extra,
    Parser,
};

pub(super) fn semantic_section<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTSemanticSection<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let open_brace = just(SleighToken::Control(SleighControlCharacter::OpenBrace));
    let close_brace = just(SleighToken::Control(SleighControlCharacter::CloseBrace));
    open_brace
        .ignore_then(any().and_is(close_brace.not()).repeated())
        .then_ignore(close_brace)
        .map_with(|_, extra| {
            (
                SASTSemanticSection {
                    actions: (SASTPCode { expressions: vec![] }, extra.span()),
                },
                extra.span(),
            )
        })
}
