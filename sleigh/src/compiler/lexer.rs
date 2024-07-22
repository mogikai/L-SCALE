use std::fmt::Debug;
use std::ops::Range;

use ariadne::{
    Label,
    Report,
    ReportKind,
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
    inline_whitespace,
    int,
    newline,
};
use chumsky::{
    extra,
    IterParser,
    ParseResult,
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
    CompilerInputCache,
    InputId,
    StringId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SleighTokens(Vec<Spanned<SleighToken>>);

impl SleighTokens {
    pub fn extend(
        &mut self,
        iter: impl IntoIterator<Item = Spanned<SleighToken>>,
    ) {
        self.0.extend(iter)
    }

    pub fn as_slice(&self) -> &[Spanned<SleighToken>] {
        self.0.as_slice()
    }

    pub fn input_id(&self) -> InputId {
        let first = self.0.first().unwrap().1.input_id;
        let last = self.0.last().unwrap().1.input_id;
        assert_eq!(first, last);
        first
    }

    pub fn iter(&self) -> impl Iterator<Item = &Spanned<SleighToken>> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Spanned<SleighToken>> {
        self.0.iter_mut()
    }
}

impl IntoIterator for SleighTokens {
    type Item = Spanned<SleighToken>;
    type IntoIter = <Vec<(SleighToken, InputSpan)> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<Vec<Spanned<SleighToken>>> for SleighTokens {
    fn from(value: Vec<Spanned<SleighToken>>) -> Self {
        Self(value)
    }
}

pub type ParserInput<'tokens> = SpannedInput<SleighToken, InputSpan, &'tokens [Spanned<SleighToken>]>;
impl SleighTokens {
    pub fn spanned(&self) -> ParserInput {
        self.0
            .spanned(InputSpan::new(self.0.last().map(|(_, span)| span.input_id).unwrap(), self.0.len()..self.0.len()))
    }
}

#[derive(Debug, Default)]
pub struct SleighLexer {}

impl SleighLexer {
    pub fn tokenize(
        &self,
        mut input_cache: &mut CompilerInputCache,
        context: &mut CompilerContext,
        input_id: InputId,
    ) -> Result<SleighTokens, SleighCompilerError> {
        // TODO: Return parsed tokens, and all encountered errors, instead of simply erroring out.
        let source_data = input_cache.input_source(input_id)?;
        let parsed = SleighLexer::tokenize_string(source_data.text(), context, input_id, 0);
        if !parsed.has_errors() {
            Ok(parsed.into_output().unwrap())
        } else {
            let mut diagnostic = vec![];
            let errors = parsed
                .into_errors()
                .into_iter()
                .map(|e| e.into_owned())
                .collect::<Vec<_>>();
            for err in errors.into_iter() {
                let error_span = err.span();
                let report = Report::build(ReportKind::Error, input_id, error_span.start())
                    .with_message("Tokenization Error")
                    .with_label(Label::new((input_id, error_span.into_range())).with_message(err.reason().to_string()))
                    .finish();
                report.write(&mut input_cache, &mut diagnostic).unwrap();
            }
            Err(SleighCompilerError::LexerError(String::from_utf8(diagnostic).unwrap()))
        }
    }

    /// Tokenize the string `data` and assume it was extracted from input `input_id`.
    pub fn tokenize_string<'src>(
        data: &'src str,
        context: &mut CompilerContext,
        input_id: InputId,
        offset: usize,
    ) -> ParseResult<SleighTokens, Rich<'src, char>> {
        lexer(input_id, offset).parse_with_state(data, context)
    }
}

pub type SleighLexerExtra<'a> = extra::Full<Rich<'a, char>, CompilerContext, ()>;

pub fn lexer<'src>(
    input_id: InputId,
    offset: usize,
) -> impl Parser<'src, &'src str, SleighTokens, SleighLexerExtra<'src>> {
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
        .map_with(move |tok, extra| {
            let range = extra.span().into_range();
            (tok, InputSpan::new(input_id, range.start + offset..range.end + offset))
        })
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .or(end().map(|_| Vec::new()))
        .map(SleighTokens)
}

fn sleigh_ident<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
    sleigh_ident_raw()
        .map_with(intern_token)
        .map(SleighToken::Ident)
}

fn sleigh_whitespace<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
    inline_whitespace()
        .at_least(1)
        .to(SleighToken::Whitespace)
        .or(newline().repeated().at_least(1).to(SleighToken::Newline))
}

fn sleigh_ident_raw<'src>() -> impl Parser<'src, &'src str, &'src str, SleighLexerExtra<'src>> + Clone {
    any()
        .filter(|c: &char| c.is_ascii_alphabetic() || *c == '.' || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
                .repeated(),
        )
        .to_slice()
}

fn sleigh_number<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
    let hex_int = just("0x")
        .ignore_then(just('0').repeated().then(int(16).or_not()).to_slice())
        .map_with(intern_token)
        .map(SleighToken::Hexadecimal);
    let bin_int = just("0b")
        .ignore_then(just('0').repeated().then(int(2).or_not()).to_slice())
        .map_with(intern_token)
        .map(SleighToken::Binary);
    let dec_int = int(10).map_with(intern_token).map(SleighToken::Decimal);
    choice((hex_int, bin_int, dec_int))
}
fn intern_token<'a, O: AsRef<str>>(
    stringy: O,
    extra: &mut MapExtra<'a, '_, &'a str, SleighLexerExtra<'a>>,
) -> StringId {
    extra.state().intern(&stringy)
}

/// Parse a sleigh comment. The comment starts with `#` and ends with a newline character.
fn sleigh_comment<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
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
fn sleigh_string<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
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
fn sleigh_macro<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
    just('@').to(SleighToken::MacroBegin).or(just("$(")
        .ignore_then(
            sleigh_ident_raw()
                .map_with(intern_token)
                .map(SleighToken::MacroExpansion),
        )
        .then_ignore(just(')')))
}

fn sleigh_operations<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
    // The single character tokens need to be parsed after the multi character tokens.
    choice([
        just("!=").to(SleighToken::Operation(SleighOperation::NotEquals)),
        just("$and").to(SleighToken::Operation(SleighOperation::BitAnd)),
        just("$or").to(SleighToken::Operation(SleighOperation::BitOr)),
        just("$xor").to(SleighToken::Operation(SleighOperation::BitXor)),
        just("f+").to(SleighToken::Operation(SleighOperation::FPlus)),
        just("f-").to(SleighToken::Operation(SleighOperation::FMinus)),
        just("f*").to(SleighToken::Operation(SleighOperation::FStar)),
        just("f/").to(SleighToken::Operation(SleighOperation::FDivision)),
        just("f<").to(SleighToken::Operation(SleighOperation::FLessThan)),
        just("f>").to(SleighToken::Operation(SleighOperation::FGreaterThan)),
        just("f==").to(SleighToken::Operation(SleighOperation::FEquals)),
        just("f!=").to(SleighToken::Operation(SleighOperation::FNotEquals)),
        just("s/").to(SleighToken::Operation(SleighOperation::SDivision)),
        just("s%").to(SleighToken::Operation(SleighOperation::SRemainder)),
        just("s<").to(SleighToken::Operation(SleighOperation::SLessThan)),
        just("s>").to(SleighToken::Operation(SleighOperation::SGreaterThan)),
    ])
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

fn sleigh_control_character<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
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

fn sleigh_keyword<'src>() -> impl Parser<'src, &'src str, SleighToken, SleighLexerExtra<'src>> + Clone {
    choice([
        just("attach").to(SleighToken::Keyword(SleighKeyword::Attach)),
        just("variables").to(SleighToken::Keyword(SleighKeyword::Variables)),
        just("values").to(SleighToken::Keyword(SleighKeyword::Values)),
        just("names").to(SleighToken::Keyword(SleighKeyword::Names)),
        just("context").to(SleighToken::Keyword(SleighKeyword::Context)),
        just("macro").to(SleighToken::Keyword(SleighKeyword::Macro)),
        just("include").to(SleighToken::Keyword(SleighKeyword::Include)),
        just("defined").to(SleighToken::Keyword(SleighKeyword::Defined)),
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
        just("ifdef").to(SleighToken::Keyword(SleighKeyword::IfDef)),
        just("ifndef").to(SleighToken::Keyword(SleighKeyword::IfNdef)),
        just("if").to(SleighToken::Keyword(SleighKeyword::If)),
        just("goto").to(SleighToken::Keyword(SleighKeyword::Goto)),
        just("undef").to(SleighToken::Keyword(SleighKeyword::Undefine)),
        just("else").to(SleighToken::Keyword(SleighKeyword::Else)),
        just("endif").to(SleighToken::Keyword(SleighKeyword::EndIf)),
        just("big").to(SleighToken::Keyword(SleighKeyword::Big)),
        just("little").to(SleighToken::Keyword(SleighKeyword::Little)),
        just("space").to(SleighToken::Keyword(SleighKeyword::Space)),
    ])
}
