use std::num::TryFromIntError;

use bitflags::bitflags;
use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode};

pub mod encoding;

#[derive(Debug, Copy, Clone)]
pub enum MemoryError {
    ConversionError,
    AlreadyMapped,
    NotMapped,
    NotAligned,
    ZeroSize,
    InvalidPermission,
    InvalidSize,
    Unknown,
    ProcessAlreadyExists,
    LayoutNotFound,
    OutOfMemory,
    GuardRegionAccess { address: EVAddr, guard_callback_id: usize },
    InvalidAlignment,
    NotContinuous,
    InvalidArguments,
    Overlap,
    IncompleteWrite(usize),
}

pub trait MMU {
    fn mem_map(
        &mut self,
        address: EPAddr,
        size: usize,
        perms: Permission,
    ) -> Result<(), MemoryError>;

    fn mem_unmap(
        &mut self,
        address: EPAddr,
        size: usize,
    ) -> Result<(), MemoryError>;

    fn mem_read_bytes(
        &self,
        address: EPAddr,
        len: usize,
    ) -> Result<Vec<u8>, MemoryError>;

    fn mem_read_bytes_down(
        &self,
        address: EPAddr,
        len: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        self.mem_read_bytes(address - len, len)
    }

    fn mem_write_bytes(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError>;

    fn mem_write_bytes_down(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError> {
        self.mem_write_bytes(address - data.len(), data)
    }

    fn flush_tlb(
        &mut self,
    ) -> Result<(), MemoryError>;
}

bitflags! {
pub struct Permission : u32 {
        const NONE = 0;
        const READ = 1;
        const WRITE = 2;
        const EXEC = 4;
        const ALL = Self::READ.bits | Self::WRITE.bits | Self::EXEC.bits;
    }
}

bitflags! {
    #[allow(non_camel_case_types)]
    pub struct
    MREMAP_FLAGS: u64 {
        const MREMAP_MAYMOVE = 1;
        const MREMAP_FIXED = 2;
        const MREMAP_DONTUNMAP = 4;
    }
}

impl TryFrom<u64> for Permission {
    type Error = MemoryError;

    fn try_from(perm: u64) -> Result<Permission, MemoryError> {
        Permission::from_bits(u32::try_from(perm)?).ok_or(MemoryError::InvalidPermission)
    }
}

impl From<TryFromIntError> for MemoryError {
    fn from(value: TryFromIntError) -> Self {
        todo!()
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum MemoryAction {
    READ,
    WRITE,
    FETCH,
    READ_UNMAPPED,
    WRITE_UNMAPPED,
    FETCH_UNMAPPED,
    WRITE_PROT,
    READ_PROT,
    FETCH_PROT,
    READ_AFTER,
}
