use chumsky::error::Rich;
use chumsky::prelude::{
    any,
    just,
};
use chumsky::{
    extra,
    Parser,
};

use crate::compiler::ast::constructor::disassembly_section::SASTDisassemblySection;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};

pub(super) fn disassembly_section<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTDisassemblySection<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let open_bracket = just(SleighToken::Control(SleighControlCharacter::OpenBracket));
    let close_bracket = just(SleighToken::Control(SleighControlCharacter::CloseBracket));
    open_bracket
        .ignore_then(any().and_is(close_bracket.not()).repeated())
        .then_ignore(close_bracket)
        .or_not()
        .map_with(|actions, extra| {
            (
                SASTDisassemblySection {
                    actions: ("", extra.span()),
                },
                extra.span(),
            )
        })
}
