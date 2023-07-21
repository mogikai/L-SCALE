use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::mmu::{MemoryError, MMU};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;
use crate::emulator::system::memory::Memory;

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
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        use itertools::Itertools;
        self.translate(address, size)?
            .iter()
            .map(|tlb_entry| {
                self.mmu
                    .mem_read_bytes(tlb_entry.physical_address, tlb_entry.size)
            })
            .flatten_ok()
            .try_collect()
    }

    pub fn write_bytes(
        &mut self,
        address: EVAddr,
        bytes: &[u8],
    ) -> Result<usize, MemoryError> {
        let tlb_entries = self.translate(address, bytes.len())?;

        let mut already_written = 0;
        for entry in tlb_entries {
            self.mmu
                .mem_write_bytes(entry.physical_address, &bytes[already_written..already_written + entry.size])
                .map_err(|_| MemoryError::IncompleteWrite(already_written))?;
            already_written += entry.size;
        }
        Ok(already_written)
    }
}
