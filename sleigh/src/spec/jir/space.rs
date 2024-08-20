use crate::new_typed_index_table_id;
use crate::spec::jir::common::JIRSize;

pub struct JIRSpace {
    pub r#type: JIRSpaceType,
    /// The number of bytes an address in the space takes up
    pub address_size: u64,
    /// The size of data in bytes accessed by an address.
    pub wordsize: JIRSize,
}

new_typed_index_table_id! { pub struct JIRSpaceId; }

pub enum JIRSpaceType {
    Ram,
    Rom,
    Register,
}
