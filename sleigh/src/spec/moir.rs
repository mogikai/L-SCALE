use crate::spec::moir::common::MOIRSize;

pub mod address_space;
pub mod bitrange_register;
pub mod code;
pub mod common;
pub mod constructor;
pub mod context;
pub mod custom_op;
pub mod endianness;
pub mod field;
pub mod r#macro;
pub mod scope;
pub mod table;
pub mod token;
pub mod varnode;
pub mod actions;

pub const MOIR_ADDRESS_SIZE: MOIRSize = 8;
