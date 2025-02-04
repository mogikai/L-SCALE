use crate::Spanned;
use crate::ast::define::SASTBitrangeRegister;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    whitespace_padded,
};
use crate::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
};
use chumsky::{
    IterParser,
    Parser,
};

pub fn bitrange_register<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Vec<Spanned<SASTBitrangeRegister>>, SleighParserExtra<'tokens>> {
    let number_pair = sleigh_control(SleighControlCharacter::OpenBracket)
        .ignore_then(sleigh_number())
        .then_ignore(sleigh_control(SleighControlCharacter::Comma))
        .then(sleigh_number())
        .then_ignore(sleigh_control(SleighControlCharacter::CloseBracket));

    let register = sast_ident_spanned() // bitrange register name
        .then_ignore(sleigh_operation(SleighOperation::Equals))
        .then(sast_ident_spanned()) // source varnode
        .then(number_pair)
        .map(|((name, source_varnode), (offset, size))| SASTBitrangeRegister {
            name,
            source_varnode,
            offset,
            size,
        })
        .map_with(spanned);

    sleigh_keyword(SleighKeyword::BitRange)
        .ignore_then(whitespace_padded(register).repeated().collect::<Vec<_>>())
        .boxed()
}
