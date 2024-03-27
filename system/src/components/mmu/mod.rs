pub mod tlb_entry;

use mogi_utilities::addresses::EPAddr;
use mogi_utilities::error::memory::MemoryError;

use std::num::TryFromIntError;

use bitflags::bitflags;
use byteorder::ByteOrder;
use thiserror::Error;

// TODO: Extract memory error from utilities into system crate?

pub type MmuResult<T> = Result<T, MemoryError>;

pub trait MMU {
    fn alloc_physical(
        &mut self,
        address: EPAddr,
        size: usize,
        perms: Permission,
    ) -> MmuResult<()>;

    fn dealloc_physical(
        &mut self,
        address: EPAddr,
        size: usize,
    ) -> MmuResult<()>;

    /// Reads the specified amount of bytes from the emulated physical memory, starting at `address`.
    ///
    /// # Arguments
    ///
    /// * `address`: The emulated physical address to start reading from.
    /// * `len`: The amount of bytes to read
    ///
    /// returns: A vector of bytes that were read from memory,
    fn read_physical(
        &self,
        address: EPAddr,
        len: usize,
    ) -> MmuResult<Vec<u8>>;

    /// Does the same thing as [Self::read_physical], but starts at
    /// the end of the memory area, and reads downwards.
    fn read_physical_down(
        &self,
        address: EPAddr,
        len: usize,
    ) -> MmuResult<Vec<u8>> {
        self.read_physical(address - len, len)
    }

    /// Writes the provided data to the emulated physical memory
    /// starting at the provided emulated physical address.
    ///
    /// # Arguments
    ///
    /// * `address`: The emulated physical address to write the data to
    /// * `data`: The data to be written.
    ///
    /// returns: The amount of data written. If a failure occurred, a [MemoryError] is returned.
    fn write_physical(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> MmuResult<usize>;

    /// Does the same thing as [Self::write_physical], but starts writing at the end
    /// of the memory and writes downwards.
    fn write_physical_down(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> MmuResult<usize> {
        self.write_physical(address - data.len(), data)
    }

    fn flush_tlb(&mut self) -> MmuResult<()>;
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct Permission : u32 {
        const NONE = 0;
        const READ = 1;
        const WRITE = 2;
        const EXEC = 4;
        const ALL = Self::READ.bits() | Self::WRITE.bits() | Self::EXEC.bits();
    }
}

bitflags! {
    #[allow(non_camel_case_types)]
    #[derive(Debug)]
    pub struct MREMAP_FLAGS: u64 {
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
