use chumsky::prelude::{
    any,
    just,
};
use chumsky::primitive::none_of;
use chumsky::{
    select,
    IterParser,
    Parser,
};

use crate::compiler::ast::constructor::display_section::SASTDisplaySection;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sleigh_control,
    sleigh_ident,
    sleigh_keyword,
    sleigh_operation,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighKeyword::Is;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighOperation,
    SleighToken,
};

pub fn display_section<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDisplaySection>, SleighParserExtra<'tokens>> {
    let start_of_display_section = sleigh_control(SleighControlCharacter::Colon);
    let end_of_display_section = sleigh_keyword(Is);

    // Parse the display string but then rewind, so we can parse mnemonic and identifiers.
    let display = any()
        .and_is(end_of_display_section.clone().not())
        .map(|tok: SleighToken| tok.to_mnemonic().to_string())
        .repeated()
        .collect::<Vec<String>>()
        .map(|s| s.into_iter().collect::<String>())
        .map_with(spanned)
        .rewind()
        .boxed();
    // let display = any().rewind().map(|_| String::new()).map_with(spanned);

    // Parse mnemonic, removing any `^` and rewind.
    // If we encounter a whitespace in the beginning, the mnemonic is empty.
    let mnemonic = just(SleighToken::Whitespace)
        .to(String::new())
        .map_with(spanned)
        .or(none_of(SleighToken::Whitespace)
            .then_ignore(sleigh_operation(SleighOperation::Hat).or_not())
            .map(|token: SleighToken| token.to_mnemonic().to_string())
            .repeated()
            .collect::<Vec<_>>()
            .map(|element| element.into_iter().collect::<String>())
            .map_with(spanned))
        .rewind()
        .boxed();
    // let mnemonic = any().rewind().map(|_| String::new()).map_with(spanned);

    let not_ident = any().and_is(
        select! {
            SleighToken::Ident(_) => (),
            SleighToken::Keyword(Is) => (),
        }
        .not(),
    );

    let identifiers = sleigh_ident()
        .padded_by(not_ident.clone().repeated())
        .map_with(spanned)
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end_of_display_section.clone())
        .boxed();

    let no_identifiers = not_ident
        .repeated()
        .map(|_| Vec::new())
        .then_ignore(end_of_display_section);

    start_of_display_section
        .ignore_then(display)
        .then(mnemonic)
        .then(identifiers.or(no_identifiers))
        .map(|((display_str, mnemonic), identifiers)| SASTDisplaySection {
            mnemonic,
            identifiers,
            display_str,
        })
        .map_with(spanned)
        .boxed()
}

#[cfg(test)]
mod test {
    use crate::compiler::parser::test::TestResult;

    use super::*;

    fn test_display_section(input: &str) -> TestResult {
        use crate::compiler::parser::test::{
            report_errors,
            test_parser,
        };
        test_parser(input, |cache, context, tokens| {
            report_errors(cache, display_section().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn empty_display_section_parses() -> TestResult {
        test_display_section(":    is")
    }

    #[test]
    fn simple_display_section_parses() -> TestResult {
        test_display_section(":MNEMONIC is")
    }
}
