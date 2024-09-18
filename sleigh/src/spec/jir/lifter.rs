use crate::spec::jir::code::JIRInstruction;
use thiserror::Error;

pub type JIRLifterResult<T = ()> = Result<T, JIRLifterError>;

pub trait JIRLifter {
    /// Lift a single instruction if possible.
    /// Returns the number of bytes that were consumed, and the lifted instructions
    /// One target instruction corresponds to multiple lifted instructions most of the time.
    fn lift(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, Vec<JIRInstruction>)>;

    /// Does the same as [lift], but tries to lift as many instructions as possible
    fn lift_many(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, ())>;

    fn disasm(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, String)>;

    fn disasm_many(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, Vec<String>)>;
}

#[derive(Debug, Error)]
pub enum JIRLifterError {
    #[error("An unknown error occurred")]
    Unknown,
}
