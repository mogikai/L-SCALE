use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::pcode;
use crate::compiler::parser::{
    SleighParserExtra,
    inline_whitespace_padded,
    parenthesized,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

pub(super) fn pcode_macro<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeMacro>, SleighParserExtra<'tokens>> {
    let ident = inline_whitespace_padded(sast_ident_spanned());

    let argument = inline_whitespace_padded(sast_ident_spanned());

    let arguments = parenthesized(
        argument
            .separated_by(sleigh_control(SleighControlCharacter::Comma))
            .collect::<Vec<_>>(),
    )
    .map_with(spanned);

    let pcode = inline_whitespace_padded(pcode().delimited_by(
        sleigh_control(SleighControlCharacter::OpenBrace),
        sleigh_control(SleighControlCharacter::CloseBrace),
    ));

    sleigh_keyword(SleighKeyword::Macro)
        .ignore_then(ident)
        .then(arguments)
        .then(pcode)
        .map(|((ident, arguments), pcode)| SASTPCodeMacro {
            ident,
            arguments,
            pcode,
        })
        .map_with(spanned)
        .boxed()
}
