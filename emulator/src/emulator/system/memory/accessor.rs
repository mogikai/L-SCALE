use bitflags::Flags;
use byteorder::ByteOrder;
use mogi_system::components::mmu::tlb_entry::TLBEntry;
use mogi_system::components::mmu::{
    Permission,
    MMU,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::error::memory::MemoryError;

use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::memory::Memory;

#[derive(Debug)]
pub struct MemoryAccessor<'a, Endianness: ByteOrder, MmuT: MMU> {
    pub(super) memory: &'a mut Memory<Endianness>,
    pub(super) mmu: &'a mut MmuT,
    pub(super) task_id: TaskId,
}

impl<Endianness: ByteOrder, MmuT: MMU> MemoryAccessor<'_, Endianness, MmuT> {
    /// Returns a list of physical regions that correspond to the
    /// translated virtual memory region address..address+size
    fn translate(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<Vec<TLBEntry>, MemoryError> {
        self.memory
            .translate_vaddr_range(self.task_id, address, size)
    }

    pub fn read_bytes(
        &mut self,
        address: EVAddr<u8>,
        size: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        use itertools::Itertools;
        let mut buf = vec![0u8; size];
        let mut start = 0;
        for tlb_entry in self.translate(address, size)? {
            let addr = tlb_entry.physical_address;
            let len = tlb_entry.size;
            self.mmu.read_physical(addr, &mut buf[start..start+len])?;
            start += len;
        }
        Ok(buf)
    }

    pub fn write_bytes(
        &mut self,
        address: EVAddr<u8>,
        bytes: &[u8],
    ) -> Result<usize, MemoryError> {
        let tlb_entries = self.translate(address, bytes.len())?;

        let mut already_written = 0;
        for entry in tlb_entries {
            if !entry.permissions.contains(Permission::WRITE) {
                Err(MemoryError::InvalidPermission)?
            }
            self.mmu
                .write_physical(entry.physical_address, &bytes[already_written..already_written + entry.size])
                .map_err(|_| MemoryError::IncompleteWrite(already_written))?;
            already_written += entry.size;
        }
        Ok(already_written)
    }

    pub fn write_bytes_unchecked(
        &mut self,
        address: EVAddr,
        bytes: &[u8],
    ) -> Result<usize, MemoryError> {
        let tlb_entries = self.translate(address, bytes.len())?;

        let mut already_written = 0;
        for entry in tlb_entries {
            self.mmu
                .write_physical(entry.physical_address, &bytes[already_written..already_written + entry.size])
                .map_err(|_| MemoryError::IncompleteWrite(already_written))?;
            already_written += entry.size;
        }
        Ok(already_written)
    }
}
