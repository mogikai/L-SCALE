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
use crate::compiler::parser::{
    sleigh_control,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};

pub fn disassembly_section<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDisassemblySection>, SleighParserExtra<'tokens>> {
    let open_bracket = whitespace_padded(sleigh_control(SleighControlCharacter::OpenBracket));
    let close_bracket = whitespace_padded(sleigh_control(SleighControlCharacter::CloseBracket));
    open_bracket
        .ignore_then(any().and_is(close_bracket.clone().not()).repeated())
        .then_ignore(close_bracket)
        .or_not()
        .map_with(|actions, extra| (SASTDisassemblySection { actions }, extra.span()))
}
