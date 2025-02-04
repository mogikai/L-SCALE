use crate::Spanned;
use crate::ast::define::{
    SASTAddressSpace,
    SASTAddressSpaceAttribute,
    SASTAddressSpaceType,
};
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    sleigh_whitespace,
};
use crate::tokens::{
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
    IterParser,
    Parser,
    select,
};

pub fn address_space<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTAddressSpace>, SleighParserExtra<'tokens>> {
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
        .map_with(spanned)
        .boxed();

    let address_space = sleigh_keyword(SleighKeyword::Space)
        .ignore_then(sleigh_whitespace())
        .then(sast_ident_spanned())
        .then(sleigh_whitespace())
        .map(|a| a.0.1) // remove () (is there a better way?)
        .then(attrs)
        .validate(|(name, attributes), _, emitter| {
            // check if mandatory size attribute exists
            if attributes
                .iter()
                .find_map(|attribute| {
                    if let SASTAddressSpaceAttribute::Size(size) = **attribute {
                        Some((size, attribute.span().clone()))
                    } else {
                        None
                    }
                })
                .is_none()
            {
                emitter.emit(Rich::custom(attributes.span().clone(), "Missing size attribute"));
            }

            SASTAddressSpace {
                name,
                attributes: attributes.into_value(),
            }
        })
        .map_with(spanned);

    address_space.boxed()
}
