use crate::addresses::EVAddr;
use std::num::TryFromIntError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum MemoryError {
    #[error("The memory Could not be converted into the required type")]
    ConversionError,
    #[error("Part of the memory that was attempted to map is already mapped in a different mapping")]
    AlreadyMapped,
    #[error("The requested memory is not mapped")]
    NotMapped,
    #[error("The memory address or size is not aligned")]
    InvalidAlignment,
    #[error("The size must not be zero")]
    ZeroSize,
    #[error("The permissions of the memory region do not allow the requested operation")]
    InvalidPermission,
    #[error("The size of the supplied memory is not sufficient for the requested operation")]
    InsufficientSize,
    #[error("The process already exists (i.e. it already has a memory layout assigned")]
    ProcessAlreadyExists,
    #[error("There is no more virtual physical memory available")]
    OutOfMemory,
    #[error("TODO")]
    GuardRegionAccess { address: EVAddr, guard_callback_id: usize },
    #[error("The requested memory region is not continuously mapped (i.e. it contains unmapped addresses)")]
    NotContinuous,
    #[error("One or more arguments are invalid")]
    InvalidArguments,
    #[error("The mapping overlaps with itself")]
    MappingOverlaps,
    #[error("The requested write could only write {0} bytes")]
    IncompleteWrite(usize),
    #[error("An unknown error has occurred")]
    Unknown,
}

impl From<TryFromIntError> for MemoryError {
    fn from(value: TryFromIntError) -> Self {
        todo!()
    }
}
