use crate::compiler::ast::define::{
    SASTAddressSpace,
    SASTAddressSpaceAttributes,
    SASTAddressSpaceType,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    sleigh_whitespace,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use chumsky::error::Rich;
use chumsky::primitive::{
    choice,
    just,
};
use chumsky::{
    select,
    IterParser,
    Parser,
};

// for parsing only
#[derive(Debug, Clone, Copy)]
pub enum SASTAddressSpaceAttribute {
    Type(SASTAddressSpaceType),
    Size(u64),
    Wordsize(u64),
    IsDefault,
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
        .map(SASTAddressSpaceAttribute::Type)
        .map_with(spanned);

    let attr_size = sleigh_keyword(SleighKeyword::Size)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map(SASTAddressSpaceAttribute::Size)
        .map_with(spanned);

    let attr_default = sleigh_keyword(SleighKeyword::Default)
        .to(SASTAddressSpaceAttribute::IsDefault)
        .map_with(spanned);

    let attr_wordsize = sleigh_keyword(SleighKeyword::WordSize)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map(SASTAddressSpaceAttribute::Wordsize)
        .map_with(spanned);

    let attrs = choice((attr_type, attr_size, attr_default, attr_wordsize))
        .then(sleigh_whitespace())
        .map(|a| a.0)
        .repeated()
        .collect::<Vec<_>>()
        .map_with(spanned);

    let address_space = sleigh_keyword(SleighKeyword::Space)
        .ignore_then(sleigh_whitespace())
        .then(sast_ident_spanned())
        .then(sleigh_whitespace())
        .map(|a| a.0 .1) // remove () (is there a better way?)
        .then(attrs)
        .validate(|(name, (attributes, span)), _, emitter| {
            // check if mandatory size attribute exists
            if attributes
                .iter()
                .find_map(|(attribute, span)| {
                    if let SASTAddressSpaceAttribute::Size(size) = attribute {
                        Some((*size, span.clone()))
                    } else {
                        None
                    }
                })
                .is_none()
            {
                emitter.emit(Rich::custom(span.clone(), "Missing size attribute"));
            }

            let mut attr_struct = SASTAddressSpaceAttributes {
                size: None,
                space_type: None,
                wordsize: None,
                default: None,
            };

            // fill in others

            for (attribute, span) in attributes.into_iter() {
                match attribute {
                    SASTAddressSpaceAttribute::Wordsize(wordsize) => attr_struct.wordsize = Some((wordsize, span)),
                    SASTAddressSpaceAttribute::Type(stype) => attr_struct.space_type = Some((stype, span)),
                    SASTAddressSpaceAttribute::Size(size) => attr_struct.size = Some((size, span)), // override duplicates
                    SASTAddressSpaceAttribute::IsDefault => attr_struct.default = Some((true, span)),
                }
            }

            SASTAddressSpace {
                name,
                attributes: (attr_struct, span),
            }
        })
        .map_with(spanned);

    address_space
}
