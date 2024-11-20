use chumsky::Parser;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::constructor::bit_pattern_section::bit_pattern_section;
use crate::compiler::parser::constructor::disassembly_section::disassembly_section;
use crate::compiler::parser::constructor::display_section::display_section;
use crate::compiler::parser::constructor::semantic_section::semantic_section;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    SleighParserExtra,
    sleigh_ident,
};

pub(super) mod bit_pattern_section;
pub(super) mod disassembly_section;
pub(super) mod display_section;
pub(super) mod semantic_section;

pub(super) fn constructor<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTConstructor>, SleighParserExtra<'tokens>> {
    // The table can be either an identifier or it may be omitted, in which case the table referenced
    // will be the root table.
    let table = sleigh_ident().or_not().map_with(spanned);

    let display_section = display_section();
    let bit_pattern_section = bit_pattern_section();
    let disassembly_section = disassembly_section();
    let semantic_section = semantic_section();

    table
        .then(display_section)
        .then(bit_pattern_section)
        .then(disassembly_section)
        .then(semantic_section)
        .map(|((((table, display_section), bit_pattern_section), disassembly_section), semantic_section)| {
            SASTConstructor {
                table,
                display_section,
                bit_pattern_section,
                disassembly_section,
                semantic_section,
            }
        })
        .map_with(spanned)
        .boxed()
}

#[cfg(test)]
mod test {
    use crate::compiler::parser::test::TestResult;

    use super::*;

    fn test_constructor(input: &str) -> TestResult {
        use crate::compiler::parser::test::{
            report_errors,
            test_parser,
        };
        test_parser(input, |cache, context, tokens| {
            report_errors(cache, constructor().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn basic_constructor_parses() -> TestResult {
        test_constructor(":INST reg is reg=0xf { x = func(y); fpop(); }")
    }

    #[test]
    fn some_constructor_parses() -> TestResult {
        test_constructor("XmmCondSS: is imm8 { xmmTmp1_Da = cmpss(xmmTmp1_Da, xmmTmp2_Da, imm8:1); }")
    }

    #[test]
    fn constructor_with_disassembly_parses() -> TestResult {
        test_constructor("n1: one	is epsilon  [ reloc = 1 ; ] { export *[const]:1 one; }")
    }
}
