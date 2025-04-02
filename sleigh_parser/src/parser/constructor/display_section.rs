use crate::ast::constructor::display_section::{
    SASTDisplayFragment,
    SASTDisplaySection,
};
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    at_least_one_sleigh_whitespace,
    sleigh_control,
    sleigh_ident,
    sleigh_keyword,
    sleigh_operation,
};
use crate::tokens::SleighKeyword::Is;
use crate::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use crate::{
    ParserContext,
    Spanned,
};
use chumsky::input::MapExtra;
use chumsky::prelude::any;
use chumsky::{
    IterParser,
    Parser,
    select,
};

pub fn display_section<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDisplaySection>, SleighParserExtra<'tokens>> {
    let start_of_display_section = sleigh_control(SleighControlCharacter::Colon);
    let end_of_display_section = sleigh_keyword(Is);

    // Parse the display string but then rewind, so we can parse mnemonic and identifiers.
    let display = any()
        .and_is(end_of_display_section.clone().not())
        .map_with(make_display_string)
        .repeated()
        .collect::<Vec<String>>()
        .map(|s| s.into_iter().collect::<String>())
        .map_with(spanned)
        .rewind()
        .boxed();
    // let display = any().rewind().map(|_| String::new()).map_with(spanned);

    // parse literal, i.e. non-identifier or surrounded by double quotes
    let string =
        select! { SleighToken::String(string_id) => SleighToken::String(string_id) }.map_with(make_display_string);
    let not_ident = any()
        .and_is(
            select! {
                SleighToken::Ident(_) => (),
                SleighToken::Keyword(Is) => (),
                // ^ has special meaning as a non-whitespace separator
                SleighToken::Operation(SleighOperation::Hat) => (),
                SleighToken::Whitespace,
            }
            .not(),
        )
        .map_with(make_display_string);

    let literal = string
        .or(not_ident)
        .map(SASTDisplayFragment::Literal)
        .map_with(spanned);
    let identifier = sleigh_ident()
        .map(SASTDisplayFragment::Identifier)
        .map_with(spanned);
    let whitespace = at_least_one_sleigh_whitespace()
        .to(SASTDisplayFragment::Whitespace)
        .map_with(spanned);
    let hat = sleigh_operation(SleighOperation::Hat)
        .to(SASTDisplayFragment::Hat)
        .map_with(spanned);

    // TODO ignore whitespace strings here
    let fragments = literal
        .or(identifier)
        .or(whitespace)
        .or(hat)
        .repeated()
        .collect::<Vec<_>>()
        .boxed();

    start_of_display_section
        .ignore_then(display)
        .then(fragments)
        .then_ignore(end_of_display_section)
        .map(|(display_str, fragments)| SASTDisplaySection { display_str, fragments })
        .map_with(spanned)
        .boxed()
}

fn make_display_string<'tokens>(
    tok: SleighToken,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> String {
    let interner = extra.state();
    match tok {
        SleighToken::String(id) => interner.string_unchecked(id),
        SleighToken::Ident(id) => interner.string_unchecked(id),
        SleighToken::Comment(id) => interner.string_unchecked(id),
        SleighToken::Decimal(id) => interner.string_unchecked(id),
        SleighToken::Hexadecimal(id) => interner.string_unchecked(id),
        SleighToken::Binary(id) => interner.string_unchecked(id),
        SleighToken::With(id) => interner.string_unchecked(id),
        SleighToken::MacroBegin => panic!("This should not occur"),
        SleighToken::MacroExpansion(_) => panic!("This should not occur"),
        SleighToken::IncludedTokens(_) => panic!("This should not occur"),
        SleighToken::Keyword(keyword) => {
            use SleighKeyword::*;
            match keyword {
                Attach => "attach",
                Variables => "variables",
                Values => "values",
                Names => "names",
                Context => "context",
                Macro => "macro",
                Include => "include",
                Define => "define",
                Endian => "endian",
                Alignment => "alignment",
                Type => "type",
                Size => "size",
                Default => "default",
                WordSize => "wordsize",
                RamSpace => "ramspace",
                RomSpace => "romspace",
                RegisterSpace => "registerspace",
                With => "with",
                BitRange => "bitrange",
                Offset => "offset",
                PCodeOp => "pcodeop",
                Token => "token",
                Signed => "signed",
                Hex => "hex",
                Dec => "dec",
                Noflow => "noflow",
                Is => "is",
                Local => "local",
                If => "if",
                Goto => "goto",
                Undefine => "undefine",
                IfDef => "ifdef",
                Else => "else",
                EndIf => "endif",
                IfNdef => "ifndef",
                Defined => "defined",
                Big => "big",
                Little => "little",
                Space => "space",
                Build => "build",
                Call => "call",
                Return => "return",
                Export => "export",
            }
        }
        SleighToken::Control(control) => {
            use SleighControlCharacter::*;
            match control {
                SemiColon => ";",
                OpenParen => "(",
                CloseParen => ")",
                OpenBracket => "[",
                CloseBracket => "]",
                OpenBrace => "{",
                CloseBrace => "}",
                Comma => ",",
                Ellipsis => "...",
                Colon => ":",
            }
        }
        SleighToken::Operation(operation) => {
            use SleighOperation::*;
            match operation {
                Equals => "==",
                And => "&",
                DoubleAnd => "&&",
                Or => "|",
                DoubleOr => "||",
                Plus => "+",
                Minus => "-",
                Not => "!",
                Hat => "^",
                Star => "*",
                LessThan => "<",
                GreaterThan => ">",
                BitNegate => "~",
                Division => "/",
                Remainder => "%",
                NotEquals => "!=",
                BitAnd => "$and",
                BitOr => "$or",
                BitXor => "$xor",
                FPlus => "f+",
                FMinus => "f-",
                FStar => "f*",
                FDivision => "f/",
                FLessThan => "f<",
                FGreaterThan => "f>",
                FEquals => "f==",
                FNotEquals => "f!=",
                SDivision => "s/",
                SRemainder => "s%",
                SLessThan => "s<",
                SGreaterThan => "s>",
            }
        }
        SleighToken::Whitespace => " ",
        SleighToken::Newline => {
            panic!("this should not occur")
        }
    }
    .to_string()
}

#[cfg(test)]
mod test {
    use crate::parser::test::TestResult;

    use super::*;

    fn test_display_section(input: &str) -> TestResult {
        use crate::parser::test::{
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
