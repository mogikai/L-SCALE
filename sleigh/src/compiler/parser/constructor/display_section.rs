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
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use chumsky::input::MapExtra;
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

pub fn display_section<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDisplaySection>, SleighParserExtra<'tokens>> {
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
        .padded_by(not_ident.repeated())
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

fn make_display_string<'tokens>(
    tok: SleighToken,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> String {
    let interner = extra.state();
    match tok {
        SleighToken::String(id) => interner.resolve_string_id(id),
        SleighToken::Ident(id) => interner.resolve_string_id(id),
        SleighToken::Comment(id) => interner.resolve_string_id(id),
        SleighToken::Decimal(id) => interner.resolve_string_id(id),
        SleighToken::Hexadecimal(id) => interner.resolve_string_id(id),
        SleighToken::Binary(id) => interner.resolve_string_id(id),
        SleighToken::With(id) => interner.resolve_string_id(id),
        SleighToken::MacroBegin => panic!("This should not occur"),
        SleighToken::MacroExpansion(id) => panic!("This should not occur"),
        SleighToken::IncludedTokens(tokens) => panic!("This should not occur"),
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
