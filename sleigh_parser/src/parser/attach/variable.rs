use crate::Spanned;
use crate::ast::attach::SASTVariable;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_whitespace,
};
use crate::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};
use chumsky::{
    IterParser,
    Parser,
};

pub fn variable<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTVariable>, SleighParserExtra<'tokens>> {
    let fieldlist = sast_ident_spanned()
        .map(|ident| vec![ident])
        .or(sast_ident_spanned()
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(
                sleigh_control(SleighControlCharacter::OpenBracket),
                sleigh_control(SleighControlCharacter::CloseBracket),
            ))
        .boxed();

    sleigh_keyword(SleighKeyword::Variables)
        .padded_by(sleigh_whitespace())
        .ignore_then(fieldlist.clone())
        .padded_by(sleigh_whitespace())
        .then(fieldlist)
        .map(|(fields, varnodes)| SASTVariable { fields, varnodes })
        .map_with(spanned)
        .boxed()
}
