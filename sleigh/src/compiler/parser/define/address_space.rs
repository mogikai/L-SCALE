use chumsky::{error::Rich, extra, primitive::{choice, just}, select, IterParser, Parser};

use crate::compiler::{ast::{define::{SASTAddressSpace, SASTAddressSpaceAttributes, SASTAddressSpaceType, SASTDefine}, Spanned}, lexer::{InputSpan, ParserInput}, parser::{sast_ident, sast_ident_spanned, sleigh_keyword, sleigh_number, sleigh_operation, sleigh_whitespace, SleighParserExtra}, tokens::{SleighKeyword, SleighOperation, SleighToken}};

// for parsing only
#[derive(Debug, Clone, Copy)]
pub enum SASTAddressSpaceAttribute {
    Type(SASTAddressSpaceType),
    Size(usize),
    Wordsize(usize),
    IsDefault
}

pub(super) fn address_space<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTAddressSpace>, SleighParserExtra<'tokens>> {
    let space_type = select! {
        SleighToken::Keyword(SleighKeyword::RamSpace) => SASTAddressSpaceType::RamSpace,
        SleighToken::Keyword(SleighKeyword::RomSpace) => SASTAddressSpaceType::RomSpace,
        SleighToken::Keyword(SleighKeyword::RegisterSpace) => SASTAddressSpaceType::RegisterSpace,
    };

    let attr_type = sleigh_keyword(SleighKeyword::Type)
        .then(just(SleighToken::Operation(SleighOperation::Equals)))
        .ignore_then(space_type)
        .map_with(|stype, extra| (SASTAddressSpaceAttribute::Type(stype), extra.span()));

    let attr_size = sleigh_keyword(SleighKeyword::Size)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(|size, extra| (SASTAddressSpaceAttribute::Size(size as usize), extra.span()));

    let attr_default = sleigh_keyword(SleighKeyword::Default)
        .map_with(|_, extra| (SASTAddressSpaceAttribute::IsDefault, extra.span()));

    let attr_wordsize = sleigh_keyword(SleighKeyword::WordSize)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(|wordsize, extra| (SASTAddressSpaceAttribute::Wordsize(wordsize as usize), extra.span()));

    let attrs = choice((attr_type, attr_size, attr_default, attr_wordsize))
        .then(sleigh_whitespace())
        .map(|a| a.0)
        .repeated()
        .collect::<Vec<_>>()
        .map_with(|attributes, extra| (attributes, extra.span()));

    let address_space = sleigh_keyword(SleighKeyword::Space)
        .ignore_then(sleigh_whitespace())
        .then(sast_ident_spanned())
        .then(sleigh_whitespace())
        .map(|a| a.0.1) // remove () (is there a better way?)
        .then(attrs)
        .map_with(|(name, a), extra| {
            // fill in mandatory attribute (size)
            let attributes: Vec<Spanned<SASTAddressSpaceAttribute>> = a.0;
            let size = attributes.iter().find_map(|a| match a.0 {
                SASTAddressSpaceAttribute::Size(size) => Some((size, a.1.clone())),
                _ => None
            }).expect("Address space definition without size attribute!"); // TODO better error handling

            let mut attr_struct = SASTAddressSpaceAttributes {
                size,
                space_type: None,
                wordsize: None,
                default: None
            };

            // fill in others

            for (attribute, span) in attributes.iter() {
                match attribute {
                    SASTAddressSpaceAttribute::Wordsize(wordsize) => attr_struct.wordsize = Some((*wordsize, span.clone())),
                    SASTAddressSpaceAttribute::Type(stype) => attr_struct.space_type = Some((*stype, span.clone())),
                    SASTAddressSpaceAttribute::Size(size) => attr_struct.size = (*size, span.clone()), // override duplicates
                    SASTAddressSpaceAttribute::IsDefault => attr_struct.default = Some((true, span.clone())),
                }
            }

            (SASTAddressSpace {
                name,
                attributes: (attr_struct, a.1)
            }, extra.span())
        });

    address_space
}
