use chumsky::{error::Rich, extra, primitive::{choice, just}, select, IterParser, Parser};

use crate::compiler::{ast::{define::{SASTAddressSpace, SASTAddressSpaceAttributes, SASTAddressSpaceType, SASTDefine}, Spanned}, lexer::{InputSpan, ParserInput}, parser::{define::endianness::endianness, sast_ident, sleigh_number}, tokens::{SleighControlCharacter, SleighKeyword, SleighOperation, SleighToken}};

use self::{address_space::SASTAddressSpaceAttribute, alignment::alignment};
use self::address_space::address_space;

use super::{sleigh_control, sleigh_keyword, sleigh_whitespace, SleighParserExtra};

mod endianness;
mod alignment;
mod address_space;

pub(super) fn define<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDefine>, SleighParserExtra<'tokens>> {
    let endian = endianness().map(SASTDefine::Endianness);
    let alignment = alignment().map(SASTDefine::Alignment);
    let address_space = address_space().map(SASTDefine::AddressSpace);

    let definitions = choice((endian, alignment, address_space));

    sleigh_keyword(SleighKeyword::Define)
        .padded_by(sleigh_whitespace())
        .ignore_then(definitions)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|define, extra| (define, extra.span()))
}
