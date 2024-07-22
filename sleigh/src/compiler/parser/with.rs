use crate::compiler::ast::item::SASTItem;
use crate::compiler::ast::with::SASTWith;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::constructor::bit_pattern_section::bit_pattern_section;
use crate::compiler::parser::constructor::disassembly_section::disassembly_section;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::included::included_tokens;
use crate::compiler::parser::{
    sleigh_control,
    sleigh_ident,
    sleigh_keyword,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter::{
    CloseBrace,
    Colon,
    OpenBrace,
};
use crate::compiler::tokens::SleighKeyword::With;
use chumsky::primitive::choice;
use chumsky::Parser;

pub fn with_block<'tokens, P>(
    items: P
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTWith>, SleighParserExtra<'tokens>>
where
    P: Parser<'tokens, ParserInput<'tokens>, Vec<SASTItem>, SleighParserExtra<'tokens>> + Clone,
{
    // The table can be either an identifier or it may be omitted, in which case the table referenced
    // will be the root table.
    let table = whitespace_padded(sleigh_ident().or_not().map_with(spanned)).then_ignore(sleigh_control(Colon));

    let bit_pattern_section = bit_pattern_section();
    let disassembly_section = disassembly_section();
    // let items = items_parser();
    let items = whitespace_padded(sleigh_control(OpenBrace))
        .ignore_then(choice((items.clone(), included_tokens(items))))
        .then_ignore(whitespace_padded(sleigh_control(CloseBrace)));

    whitespace_padded(sleigh_keyword(With))
        .ignore_then(table)
        .then(bit_pattern_section)
        .then(disassembly_section)
        .then(items)
        .map(|(((table, bit_pattern_section), disassembly_section), items)| SASTWith {
            table,
            bit_pattern_section,
            disassembly_section,
            items,
        })
        .map_with(spanned)
}
