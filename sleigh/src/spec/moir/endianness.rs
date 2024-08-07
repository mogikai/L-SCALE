use crate::compiler::ast::define::SASTEndianness;

#[derive(Debug, strum_macros::Display, Copy, Clone)]
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
