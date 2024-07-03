#[derive(Debug)]
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
}

#[derive(Debug)]
pub enum SleighOperation {
    Equals,
    And,
    Or,
    Hat,
    NotEquals,
    LessThan,
    GreaterThan,
}

#[derive(Debug)]
pub enum SleighKeyword {
    Attach,
    Variables,
    Values,
    Names,
    Context,

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

    BitRange,
    Offset,
    PCodeOp,

    Token,
    Signed,
    Hex,
    Dec,
    Noflow,
    Is,
}

#[allow(dead_code)]
#[derive(Debug)]
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
    // e.g. TableHeader { member_of: table_ident, display: ADD rax, imm8 }
    // The display string has to be parsed into mnemonic and definitions.
    TableHeader { member_of: &'src str, display: &'src str },
    // With(table_ident)
    With(&'src str),

    // e.g. Macro(include "some_file.sinc")
    Macro(&'src str),
    // e.g. MacroExpansion(some_ident)
    MacroExpansion(&'src str),

    Keyword(SleighKeyword),
    Control(SleighControlCharacter),
    Operation(SleighOperation),
}
