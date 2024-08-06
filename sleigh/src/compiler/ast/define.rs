use super::common::SASTIdent;
use super::Spanned;

// basic definitions

#[derive(Debug)]
pub enum SASTEndianness {
    Big,
    Little,
}

#[derive(Debug, Clone, Copy)]
pub enum SASTAddressSpaceType {
    RamSpace,
    RomSpace,
    RegisterSpace, // registers are defined as offsets into this address space
}

#[derive(Debug, Clone, Copy)]
pub enum SASTAddressSpaceAttribute {
    Type(SASTAddressSpaceType),
    Size(u64), // mandatory
    Wordsize(u64),
    IsDefault,
}

#[derive(Debug)]
pub struct SASTAddressSpace {
    pub name: Spanned<SASTIdent>,
    pub attributes: Vec<Spanned<SASTAddressSpaceAttribute>>,
}

#[derive(Debug)]
pub struct SASTRegister {
    pub spacename: Spanned<SASTIdent>,
    pub offset: Spanned<u64>, // offset in register address space
    pub size: Spanned<u64>,
    pub regnames: Vec<Spanned<SASTIdent>>,
}

#[derive(Debug)]
pub struct SASTBitrangeRegister {
    pub name: Spanned<SASTIdent>,
    pub source_register: Spanned<SASTIdent>,
    pub offset: u64, // offset into source register
    pub size: u64,   // number of bits
}

#[derive(Debug)]
pub struct SASTUserPcodeOp {
    pub name: Spanned<SASTIdent>,
}

// tokens and fields

#[derive(Debug)]
pub enum SASTTokenFieldAttribute {
    Signed,
    Hex,
    Dec,
}

#[derive(Debug)]
pub struct SASTTokenField {
    pub name: Spanned<SASTIdent>,
    pub start: Spanned<u64>,
    pub end: Spanned<u64>,
    pub attributes: Vec<Spanned<SASTTokenFieldAttribute>>,
}

#[derive(Debug)]
pub struct SASTToken {
    pub name: Spanned<SASTIdent>,
    pub bits: Spanned<u64>, // number of bits of token (must be multiple of 8)
    pub endian_override: Option<Spanned<SASTEndianness>>,
    pub fields: Vec<Spanned<SASTTokenField>>,
}

// almost the same as SASTTokenFieldAttribute, but adds a noflow attribute
#[derive(Debug)]
pub enum SASTContextFieldAttribute {
    Signed,
    Hex,
    Dec,
    Noflow, // change is limited to a single instruction
}

// almost the same as SASTTokenField except for the attributes
#[derive(Debug)]
pub struct SASTContextField {
    pub name: Spanned<SASTIdent>,
    pub start: Spanned<u64>,
    pub end: Spanned<u64>,
    pub attributes: Vec<Spanned<SASTContextFieldAttribute>>,
}

#[derive(Debug)]
pub struct SASTContext {
    pub reg: Spanned<SASTIdent>,
    pub fields: Vec<Spanned<SASTContextField>>,
}

pub type SASTAlignment = u16;

#[derive(Debug)]
pub enum SASTDefine {
    Endianness(Spanned<SASTEndianness>),
    Alignment(Spanned<SASTAlignment>), // the disassembler checks against this
    AddressSpace(Spanned<SASTAddressSpace>),
    Register(Spanned<SASTRegister>),
    BitrangeRegister(Vec<Spanned<SASTBitrangeRegister>>),
    UserPcodeOp(Spanned<SASTUserPcodeOp>),
    Token(Spanned<SASTToken>),
    Context(Spanned<SASTContext>),
}
