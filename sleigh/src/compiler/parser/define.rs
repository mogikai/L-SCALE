use chumsky::primitive::choice;
use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::define::SASTDefine;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::define::endianness::endianness;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

use super::{
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
    SleighParserExtra,
};

use self::address_space::address_space;
use self::alignment::alignment;
use self::bitrange_register::bitrange_register;
use self::register::register;

mod address_space;
mod alignment;
mod bitrange_register;
mod endianness;
mod register;

pub(super) fn define<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDefine>, SleighParserExtra<'tokens>> {
    let endian = endianness().map(SASTDefine::Endianness);
    let alignment = alignment().map(SASTDefine::Alignment);
    let address_space = address_space().map(SASTDefine::AddressSpace);
    let register = register().map(SASTDefine::Register);
    let bitrange_register = bitrange_register().map(SASTDefine::BitrangeRegister);

    let definitions = choice((endian, alignment, address_space, register, bitrange_register));

    whitespace_padded(sleigh_keyword(SleighKeyword::Define))
        .ignore_then(definitions)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(spanned)
}
