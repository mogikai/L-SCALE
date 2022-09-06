use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

use crate::emulator::abstractions::cpu::CPUError;
use crate::emulator::abstractions::handles::FSError;

#[derive(Debug)]
pub enum GenericErrorKind {
    CPUError(CPUError),
    FSError(FSError),
    Unknown,
}

#[derive(Debug)]
pub struct GenericError {
    kind: GenericErrorKind,
    message: String,
}

impl Display for GenericError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "GenericError({}): {}", self.kind, self.message)
    }
}

impl Display for GenericErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for GenericError {}

impl From<FSError> for GenericError {
    fn from(e: FSError) -> Self {
        Self { kind: GenericErrorKind::FSError(e), message: "".to_string() }
    }
}

impl From<CPUError> for GenericError {
    fn from(e: CPUError) -> Self {
        Self { kind: GenericErrorKind::CPUError(e), message: "".to_string() }
    }
}