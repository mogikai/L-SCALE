use std::marker::PhantomData;
use std::mem;
use std::num::TryFromIntError;
use std::ops::Range;

use bitflags::bitflags;
use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode};

pub mod accessor;
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
}

pub trait DirectMemoryWrite<T, Endianness: ByteOrder> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError>;

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError>;
}

pub trait MemoryWrite<T: Encode<Endianness>, Endianness: ByteOrder> {
    fn write(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError>;

    fn write_down(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError> {
        Self::write(self, address - data.encoded_size(), data)
    }
}

pub trait MemoryRead<T: Decode<Endianness>, Endianness: ByteOrder> {
    fn read(
        &self,
        address: EVAddr,
    ) -> Result<T, MemoryError>;

    fn read_bounded(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<T, MemoryError>;

    fn read_n(
        &self,
        address: EVAddr,
        n: usize,
    ) -> Result<Vec<T>, MemoryError>;

    fn read_down(
        &self,
        address: EVAddr,
    ) -> Result<T, MemoryError> {
        self.read(address - T::required_mem())
    }

    fn read_down_n(
        &self,
        address: EVAddr,
        n: usize,
    ) -> Result<Vec<T>, MemoryError> {
        self.read_n(address - (n * T::required_mem()), n)
    }
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
        address: EVAddr,
        len: usize,
    ) -> Result<Vec<u8>, MemoryError>;

    fn mem_read_bytes_down(
        &self,
        address: EVAddr,
        len: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        self.mem_read_bytes(address - len, len)
    }

    fn mem_write_bytes(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError>;

    fn mem_write_bytes_down(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError> {
        self.mem_write_bytes(address - data.len(), data)
    }
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
