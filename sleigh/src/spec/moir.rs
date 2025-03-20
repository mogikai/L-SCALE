use crate::spec::moir::common::MOIRSize;

pub(crate) mod actions;
pub(crate) mod address_space;
pub(crate) mod bitrange_register;
pub(crate) mod code;
pub(crate) mod common;
pub(crate) mod constructor;
pub(crate) mod context;
pub(crate) mod custom_op;
pub(crate) mod endianness;
pub(crate) mod field;
pub(crate) mod field_extractor;
pub(crate) mod r#macro;
pub(crate) mod scope;
pub(crate) mod table;
pub(crate) mod token;
pub(crate) mod varnode;

pub(crate) const MOIR_ADDRESS_SIZE: MOIRSize = 8;
