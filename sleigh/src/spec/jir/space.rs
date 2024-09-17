use crate::spec::jir::common::JIRSize;

#[derive(Debug)]
pub struct JIRSpace {
    pub r#type: JIRSpaceType,
    /// The number of bytes an address in the space takes up
    pub address_size: JIRSize,
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
