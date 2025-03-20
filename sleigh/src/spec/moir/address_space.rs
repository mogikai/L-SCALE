use crate::spec::moir::common::MOIRSize;
use crate::{
    new_typed_index_table_id_8,
    new_typed_index_table_id_16,
};
use mogi_sleigh_parser::ast::define::SASTAddressSpaceType;
use mogi_utilities::new_map_key;

#[derive(Debug)]
/// A space refers to an address space with which the cpu can interact by storing or reading data to or from.
/// Usually there are two spaces: The RAM, and the Register space.
/// The size of the backing memory for emulation must be allocated on the fly, or by analyzing the usage.
/// For the register space, all varnodes referring to the space may be collected, to determine the correct size.
pub struct MOIRSpace {
    pub r#type: MOIRSpaceType,
    /// The number of bytes an address in the space takes up
    pub address_size: MOIRSize,
    /// The size of data in bytes accessed by an address.
    pub wordsize: MOIRSize,
}

#[derive(Debug, Copy, Clone)]
pub enum MOIRSpaceType {
    Ram,
    Rom,
    Register,
    Const,
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

new_map_key! { pub struct SpaceId; "S"; }
