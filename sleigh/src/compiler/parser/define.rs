use chumsky::{error::Rich, extra, primitive::{choice, just}, select, IterParser, Parser};

use crate::compiler::{ast::{define::{SASTAddressSpace, SASTAddressSpaceAttributes, SASTAddressSpaceType, SASTDefine}, Spanned}, lexer::{InputSpan, ParserInput}, parser::{define::endianness::endianness, sast_ident, sleigh_number}, tokens::{SleighControlCharacter, SleighKeyword, SleighOperation, SleighToken}};

use self::{address_space::SASTAddressSpaceAttribute, alignment::alignment, register::register};
use self::address_space::address_space;
use self::bitrange_register::bitrange_register;

use super::{sleigh_control, sleigh_keyword, sleigh_whitespace, SleighParserExtra};

mod endianness;
mod alignment;
mod address_space;
mod register;
mod bitrange_register;

pub(super) fn define<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDefine>, SleighParserExtra<'tokens>> {
    let endian = endianness().map(SASTDefine::Endianness);
    let alignment = alignment().map(SASTDefine::Alignment);
    let address_space = address_space().map(SASTDefine::AddressSpace);
    let register = register().map(SASTDefine::Register);
    let bitrange_register = bitrange_register().map(SASTDefine::BitrangeRegister);

    let definitions = choice((endian, alignment, address_space, register, bitrange_register));

    sleigh_keyword(SleighKeyword::Define)
        .padded_by(sleigh_whitespace())
        .ignore_then(definitions)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|define, extra| (define, extra.span()))
}
