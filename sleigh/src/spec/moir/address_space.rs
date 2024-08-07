use crate::compiler::ast::define::SASTAddressSpaceType;
use crate::new_symbol_id_type;

#[derive(Debug)]
/// A space refers to an address space with which the cpu can interact by storing or reading data to or from.
/// Usually there are two spaces: The RAM, and the Register space.
/// The size of the backing memory for emulation must be allocated on the fly, or by analyzing the usage.
/// For the register space, all varnodes referring to the space may be collected, to determine the correct size.
pub struct MOIRSpace {
    pub r#type: MOIRSpaceType,
    /// The number of bytes an address in the space takes up
    pub address_size: u64,
    /// The size of data in bytes accessed by an address.
    pub wordsize: u16,
}

#[derive(Debug)]
pub enum MOIRSpaceType {
    Ram,
    Rom,
    Register,
}

#[derive(Debug)]
pub enum MOIRSpaceIsDefault {
    Default,
    NotDefault,
}

impl From<SASTAddressSpaceType> for MOIRSpaceType {
    fn from(value: SASTAddressSpaceType) -> Self {
        match value {
            SASTAddressSpaceType::RamSpace => MOIRSpaceType::Ram,
            SASTAddressSpaceType::RomSpace => MOIRSpaceType::Rom,
            SASTAddressSpaceType::RegisterSpace => MOIRSpaceType::Register,
        }
    }
}

new_symbol_id_type! { pub struct MOIRSpaceId; }
