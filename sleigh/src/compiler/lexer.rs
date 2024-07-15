use std::fmt::{
    Debug,
    Formatter,
};
use std::ops::Range;

use ariadne::{
    Label,
    Report,
    ReportKind,
    Source,
};
use chumsky::error::Rich;
use chumsky::input::{
    Input,
    MapExtra,
    SpannedInput,
};
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
use chumsky::span::Span;
use chumsky::text::{
    int,
    newline,
    whitespace,
};
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use crate::compiler::{
    CompilerContext,
    CompilerInput,
    CompilerInputCache,
    InputId,
    StringId,
};

#[derive(Clone, Debug)]
pub struct InputSpan {
    range: Range<usize>,
    input_id: InputId,
}

impl Span for InputSpan {
    type Context = InputId;
    type Offset = usize;

    fn new(
        context: Self::Context,
        range: Range<Self::Offset>,
    ) -> Self {
        Self {
            range,
            input_id: context,
        }
    }

    fn context(&self) -> Self::Context {
        self.input_id
    }

    fn start(&self) -> Self::Offset {
        self.range.start
    }

    fn end(&self) -> Self::Offset {
        self.range.end
    }
}

impl ariadne::Span for InputSpan {
    type SourceId = InputId;

    fn source(&self) -> &Self::SourceId {
        &self.input_id
    }

    fn start(&self) -> usize {
        self.range.start
    }

    fn end(&self) -> usize {
        self.range.end
    }
}

#[derive(Debug)]
pub struct SleighTokens(Vec<Spanned<SleighToken>>);
pub type ParserInput<'tokens> = SpannedInput<SleighToken, InputSpan, &'tokens [Spanned<SleighToken>]>;
impl SleighTokens {
    pub fn spanned<'tokens>(&'tokens self) -> ParserInput<'tokens> {
        self.0
            .spanned(InputSpan::new(self.0.last().map(|(_, span)| span.input_id).unwrap(), self.0.len()..self.0.len()))
    }
}

#[derive(Debug, Default)]
pub struct SleighLexer {}

impl SleighLexer {
    pub fn tokenize<'src>(
        &self,
        input_cache: &CompilerInputCache,
        context: &mut CompilerContext,
        input_id: InputId,
    ) -> Result<SleighTokens, SleighCompilerError> {
        // TODO: Return parsed tokens, and all encountered errors, instead of simply erroring out.
        let source = input_cache.input(input_id)?;
        let parsed = lexer(input_id).parse_with_state(source.data(), context);
        let mut diagnostic = vec![];
        parsed.errors().for_each(|err| {
            let error_span = err.span();
            let report = Report::build(ReportKind::Error, input_id, error_span.start())
                .with_message("Tokenization Error")
                .with_label(Label::new((input_id, error_span.into_range())).with_message(err.reason().to_string()))
                .finish();
            report
                .write((input_id, Source::from(source.data())), &mut diagnostic)
                .unwrap();
        });
        if diagnostic.is_empty() {
            Ok(parsed.into_output().unwrap())
        } else {
            Err(SleighCompilerError::LexerError(String::from_utf8(diagnostic).unwrap()))
        }
    }
}

pub type SleighCompilerExtra<'a> = extra::Full<Rich<'a, char>, CompilerContext, ()>;

pub fn lexer<'src>(input_id: InputId) -> impl Parser<'src, &'src str, SleighTokens, SleighCompilerExtra<'src>> {
    let string = sleigh_string();

    let comment = sleigh_comment();

    let ident = sleigh_ident();

    let number = sleigh_number();

    let sleigh_macro = sleigh_macro();

    let whitespace = sleigh_whitespace();

    let operation = sleigh_operations();

    let control_character = sleigh_control_character();

    let keyword = sleigh_keyword();

    let token =
        choice((comment, string, sleigh_macro, whitespace, operation, keyword, control_character, ident, number));

    // Strings have the highest precedence, since anything contained in them must not be parsed.
    // The same goes for comments
    token
        .map_with(move |tok, extra| (tok, InputSpan::new(input_id, extra.span().into_range())))
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .or(end().map(|_| Vec::new()))
        .map(SleighTokens)
}

fn sleigh_ident<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    sleigh_ident_raw()
        .map_with(intern_token)
        .map(SleighToken::Ident)
}

fn sleigh_whitespace<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    whitespace().at_least(1).to(SleighToken::Whitespace)
}

fn sleigh_ident_raw<'src>() -> impl Parser<'src, &'src str, &'src str, SleighCompilerExtra<'src>> + Copy {
    any()
        .filter(|c: &char| c.is_ascii_alphabetic() || *c == '.' || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
                .repeated(),
        )
        .to_slice()
}

fn sleigh_number<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    let hex_int = just("0x")
        .ignore_then(just('0').repeated().then(int(16)).to_slice())
        .map_with(intern_token)
        .map(SleighToken::Hexadecimal);
    let bin_int = just("0b")
        .ignore_then(just('0').repeated().then(int(2)).to_slice())
        .map_with(intern_token)
        .map(SleighToken::Binary);
    let dec_int = int(10).map_with(intern_token).map(SleighToken::Decimal);
    choice((hex_int, bin_int, dec_int))
}
fn intern_token<'a, O: AsRef<str>>(
    stringy: O,
    extra: &mut MapExtra<'a, '_, &'a str, SleighCompilerExtra<'a>>,
) -> StringId {
    extra.state().intern(&stringy)
}

/// Parse a sleigh comment. The comment starts with `#` and ends with a newline character.
fn sleigh_comment<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    just('#')
        .ignore_then(
            any()
                .and_is(newline().not())
                .repeated()
                .at_least(1)
                .or_not()
                .to_slice(),
        )
        .then_ignore(newline().or(end()))
        .map_with(intern_token)
        .map(SleighToken::Comment)
}

/// Parse a string delimited by the character `"`. Any other character may be contained in the string.
fn sleigh_string<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    any()
        .and_is(just('"').not())
        .repeated()
        .to_slice()
        .delimited_by(just('"'), just('"'))
        .map_with(intern_token)
        .map(SleighToken::String)
}

/// Parse a macro. A macro is delimited by `@` and a newline character.
/// Also parses macro expansions of the form `$(IDENT)`
fn sleigh_macro<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    just('@')
        .ignore_then(any().and_is(newline().not()).repeated().to_slice())
        .map_with(intern_token)
        .map(SleighToken::Macro)
        .or(just("$(")
            .ignore_then(
                sleigh_ident_raw()
                    .map_with(intern_token)
                    .map(SleighToken::MacroExpansion),
            )
            .then_ignore(just(')')))
}

fn sleigh_operations<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    // The single character tokens need to be parsed after the multi character tokens.
    just("!=")
        .map(|_| SleighToken::Operation(SleighOperation::NotEquals))
        .or(just("$and").map(|_| SleighToken::Operation(SleighOperation::BitAnd)))
        .or(just("$or").map(|_| SleighToken::Operation(SleighOperation::BitOr)))
        .or(just("$xor").map(|_| SleighToken::Operation(SleighOperation::BitXor)))
        .or(just("f+").map(|_| SleighToken::Operation(SleighOperation::FPlus)))
        .or(just("f-").map(|_| SleighToken::Operation(SleighOperation::FMinus)))
        .or(just("f*").map(|_| SleighToken::Operation(SleighOperation::FStar)))
        .or(just("f/").map(|_| SleighToken::Operation(SleighOperation::FDivision)))
        .or(just("f<").map(|_| SleighToken::Operation(SleighOperation::FLessThan)))
        .or(just("f>").map(|_| SleighToken::Operation(SleighOperation::FGreaterThan)))
        .or(just("f==").map(|_| SleighToken::Operation(SleighOperation::FEquals)))
        .or(just("f!=").map(|_| SleighToken::Operation(SleighOperation::FNotEquals)))
        .or(just("s/").map(|_| SleighToken::Operation(SleighOperation::SDivision)))
        .or(just("s%").map(|_| SleighToken::Operation(SleighOperation::SRemainder)))
        .or(just("s<").map(|_| SleighToken::Operation(SleighOperation::SLessThan)))
        .or(just("s>").map(|_| SleighToken::Operation(SleighOperation::SGreaterThan)))
        .or(one_of("=&|+-/%!^*<>~").map(|c| match c {
            '=' => SleighToken::Operation(SleighOperation::Equals),
            '&' => SleighToken::Operation(SleighOperation::And),
            '|' => SleighToken::Operation(SleighOperation::Or),
            '+' => SleighToken::Operation(SleighOperation::Plus),
            '-' => SleighToken::Operation(SleighOperation::Minus),
            '/' => SleighToken::Operation(SleighOperation::Division),
            '%' => SleighToken::Operation(SleighOperation::Remainder),
            '!' => SleighToken::Operation(SleighOperation::Not),
            '^' => SleighToken::Operation(SleighOperation::Hat),
            '*' => SleighToken::Operation(SleighOperation::Star),
            '<' => SleighToken::Operation(SleighOperation::LessThan),
            '>' => SleighToken::Operation(SleighOperation::GreaterThan),
            '~' => SleighToken::Operation(SleighOperation::BitNegate),
            _ => panic!("This cannot happen"),
        }))
}

fn sleigh_control_character<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    one_of(";:()[]{},")
        .map(|c| match c {
            ';' => SleighToken::Control(SleighControlCharacter::SemiColon),
            ':' => SleighToken::Control(SleighControlCharacter::Colon),
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

fn sleigh_keyword<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighCompilerExtra<'src>> + Copy {
    choice([
        just("attach").to(SleighToken::Keyword(SleighKeyword::Attach)),
        just("variables").to(SleighToken::Keyword(SleighKeyword::Variables)),
        just("values").to(SleighToken::Keyword(SleighKeyword::Values)),
        just("names").to(SleighToken::Keyword(SleighKeyword::Names)),
        just("context").to(SleighToken::Keyword(SleighKeyword::Context)),
        just("macro").to(SleighToken::Keyword(SleighKeyword::Macro)),
        just("define").to(SleighToken::Keyword(SleighKeyword::Define)),
        just("endian").to(SleighToken::Keyword(SleighKeyword::Endian)),
        just("alignment").to(SleighToken::Keyword(SleighKeyword::Alignment)),
        just("type").to(SleighToken::Keyword(SleighKeyword::Type)),
        just("size").to(SleighToken::Keyword(SleighKeyword::Size)),
        just("default").to(SleighToken::Keyword(SleighKeyword::Default)),
        just("wordsize").to(SleighToken::Keyword(SleighKeyword::WordSize)),
        just("ram_space").to(SleighToken::Keyword(SleighKeyword::RamSpace)),
        just("rom_space").to(SleighToken::Keyword(SleighKeyword::RomSpace)),
        just("register_space").to(SleighToken::Keyword(SleighKeyword::RegisterSpace)),
        just("with").to(SleighToken::Keyword(SleighKeyword::With)),
        just("bitrange").to(SleighToken::Keyword(SleighKeyword::BitRange)),
        just("offset").to(SleighToken::Keyword(SleighKeyword::Offset)),
        just("pcodeop").to(SleighToken::Keyword(SleighKeyword::PCodeOp)),
        just("token").to(SleighToken::Keyword(SleighKeyword::Token)),
        just("signed").to(SleighToken::Keyword(SleighKeyword::Signed)),
        just("hex").to(SleighToken::Keyword(SleighKeyword::Hex)),
        just("dec").to(SleighToken::Keyword(SleighKeyword::Dec)),
        just("noflow").to(SleighToken::Keyword(SleighKeyword::Noflow)),
        just("is").to(SleighToken::Keyword(SleighKeyword::Is)),
        just("local").to(SleighToken::Keyword(SleighKeyword::Local)),
        just("if").to(SleighToken::Keyword(SleighKeyword::If)),
        just("goto").to(SleighToken::Keyword(SleighKeyword::Goto)),
    ])
}
