use std::fmt;
use std::fmt::{
    Display,
    Formatter,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SleighControlCharacter {
    // Control characters
    // Semicolon delimits the end of a definition.
    SemiColon,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Comma,
    Ellipsis,
    Colon,
}

impl SleighControlCharacter {
    pub fn to_mnemonic(&self) -> &str {
        match self {
            SleighControlCharacter::SemiColon => ";",
            SleighControlCharacter::OpenParen => "(",
            SleighControlCharacter::CloseParen => ")",
            SleighControlCharacter::OpenBracket => "{",
            SleighControlCharacter::CloseBracket => "}",
            SleighControlCharacter::OpenBrace => "[",
            SleighControlCharacter::CloseBrace => "]",
            SleighControlCharacter::Comma => ",",
            SleighControlCharacter::Ellipsis => "...",
            SleighControlCharacter::Colon => ":",
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SleighOperation {
    Equals,
    And,
    Or,
    Plus,
    Minus,
    Not,
    Hat,
    Star,
    LessThan,
    GreaterThan,
    BitNegate,
    Division,
    Remainder,

    // Multi character operations
    NotEquals,
    BitAnd,
    BitOr,
    BitXor,

    FPlus,
    FMinus,
    FStar,
    FDivision,
    FLessThan,
    FGreaterThan,
    FEquals,
    FNotEquals,

    SDivision,
    SRemainder,
    SLessThan,
    SGreaterThan,
}

impl SleighOperation {
    pub fn to_mnemonic(&self) -> &str {
        match self {
            SleighOperation::Equals => "=",
            SleighOperation::And => "&",
            SleighOperation::Or => "|",
            SleighOperation::Plus => "+",
            SleighOperation::Minus => "-",
            SleighOperation::Not => "!",
            SleighOperation::Hat => "^",
            SleighOperation::Star => "*",
            SleighOperation::LessThan => "<",
            SleighOperation::GreaterThan => ">",
            SleighOperation::BitNegate => "~",
            SleighOperation::Division => "/",
            SleighOperation::Remainder => "%",
            SleighOperation::NotEquals => "!=",
            SleighOperation::BitAnd => "$and",
            SleighOperation::BitOr => "$or",
            SleighOperation::BitXor => "$xor",
            SleighOperation::FPlus => "f+",
            SleighOperation::FMinus => "f-",
            SleighOperation::FStar => "f*",
            SleighOperation::FDivision => "f/",
            SleighOperation::FLessThan => "f<",
            SleighOperation::FGreaterThan => "f>",
            SleighOperation::FEquals => "f==",
            SleighOperation::FNotEquals => "f!=",
            SleighOperation::SDivision => "s/",
            SleighOperation::SRemainder => "s%",
            SleighOperation::SLessThan => "s<",
            SleighOperation::SGreaterThan => "s>",
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SleighKeyword {
    Attach,
    Variables,
    Values,
    Names,
    Context,

    Macro,
    Define,
    Endian,
    Alignment,
    Type,
    Size,
    Default,
    WordSize,
    RamSpace,
    RomSpace,
    RegisterSpace,
    With,

    BitRange,
    Offset,
    PCodeOp,

    Token,
    Signed,
    Hex,
    Dec,
    Noflow,
    Is,
    Local,
    If,
    Goto,
}

impl SleighKeyword {
    pub fn to_mnemonic(&self) -> &str {
        match self {
            SleighKeyword::Attach => "attach",
            SleighKeyword::Variables => "variables",
            SleighKeyword::Values => "values",
            SleighKeyword::Names => "names",
            SleighKeyword::Context => "context",
            SleighKeyword::Define => "define",
            SleighKeyword::Endian => "endian",
            SleighKeyword::Alignment => "alignment",
            SleighKeyword::Type => "type",
            SleighKeyword::Size => "size",
            SleighKeyword::Default => "default",
            SleighKeyword::WordSize => "wordsize",
            SleighKeyword::RamSpace => "ram_space",
            SleighKeyword::RomSpace => "rom_space",
            SleighKeyword::RegisterSpace => "register_space",
            SleighKeyword::With => "with",
            SleighKeyword::BitRange => "bitrange",
            SleighKeyword::Offset => "offset",
            SleighKeyword::PCodeOp => "pcodeop",
            SleighKeyword::Token => "token",
            SleighKeyword::Signed => "signed",
            SleighKeyword::Hex => "hex",
            SleighKeyword::Dec => "dec",
            SleighKeyword::Noflow => "noflow",
            SleighKeyword::Is => "is",
            SleighKeyword::Macro => "macro",
            SleighKeyword::Local => "local",
            SleighKeyword::If => "if",
            SleighKeyword::Goto => "goto",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SleighToken<'src> {
    // e.g. String(This is some string)
    String(&'src str),
    // e.g. Ident(my_identifier)
    Ident(&'src str),
    // e.g. Comment(This is a comment :>)
    Comment(&'src str),
    // e.g. Decimal(10534)
    Decimal(&'src str),
    // e.g. Hexadecimal(0x5fad)
    Hexadecimal(&'src str),
    // e.g. Binary(0b101101)
    Binary(&'src str),
    // With(table_ident)
    With(&'src str),

    // e.g. Macro(include "some_file.sinc")
    Macro(&'src str),
    // e.g. MacroExpansion(some_ident)
    MacroExpansion(&'src str),

    Keyword(SleighKeyword),
    Control(SleighControlCharacter),
    Operation(SleighOperation),
    Whitespace,
}

impl SleighToken<'_> {
    pub fn to_mnemonic(&self) -> &str {
        match self {
            SleighToken::String(s) => s,
            SleighToken::Ident(s) => s,
            SleighToken::Comment(s) => s,
            SleighToken::Decimal(s) => s,
            SleighToken::Hexadecimal(s) => s,
            SleighToken::Binary(s) => s,
            SleighToken::With(s) => s,
            SleighToken::Macro(s) => s,
            SleighToken::MacroExpansion(s) => s,
            SleighToken::Keyword(kw) => kw.to_mnemonic(),
            SleighToken::Control(c) => c.to_mnemonic(),
            SleighToken::Operation(o) => o.to_mnemonic(),
            SleighToken::Whitespace => " ",
        }
    }
}

impl Display for SleighToken<'_> {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
