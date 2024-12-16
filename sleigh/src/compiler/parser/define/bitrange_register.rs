use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::define::SASTBitrangeRegister;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
};

pub(super) fn bitrange_register<'tokens>()
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
