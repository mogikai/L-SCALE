use chumsky::error::Rich;
use chumsky::{
    extra,
    Parser,
};

use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::constructor::bit_pattern_section::bit_pattern_section;
use crate::compiler::parser::constructor::disassembly_section::disassembly_section;
use crate::compiler::parser::constructor::display_section::display_section;
use crate::compiler::parser::constructor::semantic_section::semantic_section;
use crate::compiler::parser::{
    sleigh_ident,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighToken;

mod bit_pattern_section;
mod disassembly_section;
mod display_section;
mod semantic_section;

pub(super) fn constructor<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTConstructor>, SleighParserExtra<'tokens>> {
    // The table can be either an identifier or it may be omitted, in which case the table referenced
    // will be the root table.
    let table = sleigh_ident()
        .or_not()
        .map_with(|ident, extra| (ident, extra.span()));

    let display_section = display_section();
    let bit_pattern_section = bit_pattern_section();
    let disassembly_section = disassembly_section();
    let semantic_section = semantic_section();

    table
        .then(display_section)
        .then(bit_pattern_section)
        .then(disassembly_section)
        .then(semantic_section)
        .map_with(
            |((((table, display_section), bit_pattern_section), disassembly_section), semantic_section), extra| {
                (
                    SASTConstructor {
                        table,
                        display_section,
                        bit_pattern_section,
                        disassembly_section,
                        semantic_section,
                    },
                    extra.span(),
                )
            },
        )
}
