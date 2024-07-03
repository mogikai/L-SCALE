use ariadne::{
    Label,
    Report,
    ReportKind,
    Source,
};
use chumsky::error::Rich;
use chumsky::prelude::{
    any,
    choice,
    one_of,
    skip_then_retry_until,
};
use chumsky::primitive::{
    end,
    just,
};
use chumsky::span::SimpleSpan;
use chumsky::text::{
    int,
    newline,
};
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::error::SleighCompilerError;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

pub type Span = SimpleSpan<usize>;

#[derive(Debug, Default)]
pub struct SleighLexer {}

impl SleighLexer {
    pub fn tokenize<'src>(
        &self,
        source: &'src str,
    ) -> Result<Vec<(SleighToken<'src>, Span)>, SleighCompilerError> {
        // TODO: Return parsed tokens, and all encountered errors, instead of simply erroring out.
        let parsed = lexer().parse(source);
        let mut diagnostic = vec![];
        parsed.errors().for_each(|err| {
            let report = Report::build(ReportKind::Error, (), err.span().start)
                .with_message("Tokenization Error")
                .with_label(Label::new(err.span().into_range()).with_message(err.reason().to_string()))
                .finish();
            report.write(Source::from(source), &mut diagnostic).unwrap();
        });
        if diagnostic.is_empty() {
            Ok(parsed.into_output().unwrap())
        } else {
            eprintln!("{}", std::str::from_utf8(diagnostic.as_slice()).unwrap());
            Err(SleighCompilerError::LexerError(String::from_utf8(diagnostic).unwrap()))
        }
    }
}

pub fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<(SleighToken<'src>, Span)>, extra::Err<Rich<'src, char>>> {
    let string = sleigh_string();

    let comment = sleigh_comment();

    let ident = sleigh_ident();

    let number = sleigh_number();

    let sleigh_macro = sleigh_macro();

    let operation = sleigh_operations();

    let with = sleigh_with();

    let table_header = sleigh_table_header();

    let control_character = sleigh_control_character();

    let keyword = sleigh_keyword();

    let token = choice((
        string,
        comment,
        sleigh_macro,
        with,
        keyword,
        table_header,
        control_character,
        ident,
        number,
        operation,
    ));

    // Strings have the highest precedence, since anything contained in them must not be parsed.
    // The same goes for comments
    token
        .map_with(|tok, extra| (tok, extra.span()))
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .or(end().map(|_| Vec::new()))
}

fn sleigh_ident<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    sleigh_ident_raw().map(SleighToken::Ident)
}

fn sleigh_ident_raw<'src>() -> impl Parser<'src, &'src str, &'src str, extra::Err<Rich<'src, char>>> + Copy {
    any()
        .filter(|c: &char| c.is_ascii_alphabetic() || *c == '.' || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
                .repeated(),
        )
        .to_slice()
}

fn sleigh_number<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    let hex_int = just("0x")
        .then(int(16))
        .to_slice()
        .map(SleighToken::Hexadecimal);
    let bin_int = just("0b").then(int(2)).to_slice().map(SleighToken::Binary);
    let dec_int = int(10).map(SleighToken::Decimal);
    choice((hex_int, bin_int, dec_int))
}

/// Parse a sleigh comment. The comment starts with `#` and ends with a newline character.
fn sleigh_comment<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    just('#')
        .ignore_then(
            any()
                .and_is(newline().not())
                .repeated()
                .at_least(1)
                .or_not()
                .to_slice(),
        )
        .then_ignore(newline())
        .map(SleighToken::Comment)
}

/// Parse a string delimited by the character `"`. Any other character may be contained in the string.
fn sleigh_string<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    any()
        .and_is(just('"').not())
        .repeated()
        .to_slice()
        .delimited_by(just('"'), just('"'))
        .map(SleighToken::String)
}

/// Parse a macro. A macro is delimited by `@` and a newline character.
/// Also parses macro expansions of the form `$(IDENT)`
fn sleigh_macro<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    just('@')
        .ignore_then(any().and_is(newline().not()).repeated().to_slice())
        .map(SleighToken::Macro)
        .or(just("$(")
            .ignore_then(sleigh_ident_raw().map(SleighToken::MacroExpansion))
            .then_ignore(just(')')))
}

fn sleigh_operations<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    one_of("=&|^<>")
        .map(|c| match c {
            '=' => SleighToken::Operation(SleighOperation::Equals),
            '&' => SleighToken::Operation(SleighOperation::And),
            '|' => SleighToken::Operation(SleighOperation::Or),
            '^' => SleighToken::Operation(SleighOperation::Hat),
            '<' => SleighToken::Operation(SleighOperation::LessThan),
            '>' => SleighToken::Operation(SleighOperation::GreaterThan),
            _ => panic!("This cannot happen"),
        })
        .or(just("!=").map(|_| SleighToken::Operation(SleighOperation::NotEquals)))
}

fn sleigh_control_character<'src>(
) -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    one_of(";()[]{},")
        .map(|c| match c {
            ';' => SleighToken::Control(SleighControlCharacter::SemiColon),
            '(' => SleighToken::Control(SleighControlCharacter::OpenParen),
            ')' => SleighToken::Control(SleighControlCharacter::CloseParen),
            '[' => SleighToken::Control(SleighControlCharacter::OpenBracket),
            ']' => SleighToken::Control(SleighControlCharacter::CloseBracket),
            '{' => SleighToken::Control(SleighControlCharacter::OpenBrace),
            '}' => SleighToken::Control(SleighControlCharacter::CloseBrace),
            ',' => SleighToken::Control(SleighControlCharacter::Comma),
            _ => panic!("This cannot happen"),
        })
        .or(just("...").map(|_| SleighToken::Control(SleighControlCharacter::Ellipsis)))
}

fn sleigh_table_header<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    let end_of_display_section = just("is").ignored();
    sleigh_ident_raw()
        .or(just(""))
        .then(
            just(':').ignore_then(
                any()
                    .and_is(end_of_display_section.not())
                    .repeated()
                    .to_slice(),
            ),
        )
        .map(|(member_of, display)| SleighToken::TableHeader { member_of, display })
}

fn sleigh_with<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    just("with")
        .ignore_then(sleigh_ident_raw().or(just("")).padded())
        .then_ignore(just(':').padded())
        .map(SleighToken::With)
}

fn sleigh_keyword<'src>() -> impl Parser<'src, &'src str, SleighToken<'src>, extra::Err<Rich<'src, char>>> + Copy {
    choice((
        just("attach").map(|_| SleighToken::Keyword(SleighKeyword::Attach)),
        just("variables").map(|_| SleighToken::Keyword(SleighKeyword::Variables)),
        just("values").map(|_| SleighToken::Keyword(SleighKeyword::Values)),
        just("names").map(|_| SleighToken::Keyword(SleighKeyword::Names)),
        just("context").map(|_| SleighToken::Keyword(SleighKeyword::Context)),
        just("define").map(|_| SleighToken::Keyword(SleighKeyword::Define)),
        just("endian").map(|_| SleighToken::Keyword(SleighKeyword::Endian)),
        just("alignment").map(|_| SleighToken::Keyword(SleighKeyword::Alignment)),
        just("type").map(|_| SleighToken::Keyword(SleighKeyword::Type)),
        just("size").map(|_| SleighToken::Keyword(SleighKeyword::Size)),
        just("default").map(|_| SleighToken::Keyword(SleighKeyword::Default)),
        just("wordsize").map(|_| SleighToken::Keyword(SleighKeyword::WordSize)),
        just("ram_space").map(|_| SleighToken::Keyword(SleighKeyword::RamSpace)),
        just("rom_space").map(|_| SleighToken::Keyword(SleighKeyword::RomSpace)),
        just("register_space").map(|_| SleighToken::Keyword(SleighKeyword::RegisterSpace)),
        just("bitrange").map(|_| SleighToken::Keyword(SleighKeyword::BitRange)),
        just("offset").map(|_| SleighToken::Keyword(SleighKeyword::Offset)),
        just("pcodeop").map(|_| SleighToken::Keyword(SleighKeyword::PCodeOp)),
        just("token").map(|_| SleighToken::Keyword(SleighKeyword::Token)),
        just("signed").map(|_| SleighToken::Keyword(SleighKeyword::Signed)),
        just("hex").map(|_| SleighToken::Keyword(SleighKeyword::Hex)),
        just("dec").map(|_| SleighToken::Keyword(SleighKeyword::Dec)),
        just("noflow").map(|_| SleighToken::Keyword(SleighKeyword::Noflow)),
        just("is").map(|_| SleighToken::Keyword(SleighKeyword::Is)),
    ))
}
