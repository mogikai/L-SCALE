use super::common::SASTIdent;
use super::Spanned;

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

#[derive(Debug)]
pub struct SASTAddressSpaceAttributes {
    pub space_type: Option<Spanned<SASTAddressSpaceType>>,
    pub size: Option<Spanned<u64>>, // number of bytes needed to address any location (mandatory)
    pub wordsize: Option<Spanned<u64>>, // number of bytes per address location
    pub default: Option<Spanned<bool>>, // if true, this address space will be used by the main address bus
}

#[derive(Debug)]
pub struct SASTAddressSpace {
    pub name: Spanned<SASTIdent>,
    pub attributes: Spanned<SASTAddressSpaceAttributes>,
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
pub enum SASTDefine {
    Endianness(Spanned<SASTEndianness>),
    Alignment(Spanned<u64>), // the disassembler checks against this
    AddressSpace(Spanned<SASTAddressSpace>),
    Register(Spanned<SASTRegister>),
    BitrangeRegister(Vec<Spanned<SASTBitrangeRegister>>),
    Foo(Spanned<u32>),
}
