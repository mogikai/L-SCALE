use crate::new_typed_index_table_id;
use crate::spec::jir::common::JIRSize;

#[derive(Debug)]
pub struct JIRSpace {
    pub r#type: JIRSpaceType,
    /// The number of bytes an address in the space takes up
    pub address_size: u64,
    /// The size of data in bytes accessed by an address.
    pub wordsize: JIRSize,
}

#[derive(Debug)]
pub enum JIRSpaceType {
    Ram,
    Rom,
    Register,
    Const,
    Temporary,
}
