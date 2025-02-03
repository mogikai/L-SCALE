use crate::compiler::ast::define::SASTEndianness;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, strum_macros::Display, Copy, Clone, Serialize, Deserialize)]
pub enum MOIREndianness {
    Big,
    Little,
}

impl From<SASTEndianness> for MOIREndianness {
    fn from(endianness: SASTEndianness) -> Self {
        match endianness {
            SASTEndianness::Big => MOIREndianness::Big,
            SASTEndianness::Little => MOIREndianness::Little,
        }
    }
}
