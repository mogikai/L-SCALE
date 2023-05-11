use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::mmu::accessor::MemoryAccessor;
use crate::emulator::abstractions::mmu::{Decode, MemoryError, MemoryRead, MMU};

impl<Endianness: ByteOrder, MmuT: MMU, T: Decode<Endianness>> MemoryRead<T, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn read(
        &self,
        address: EVAddr,
    ) -> Result<T, MemoryError> {
        T::decode(self.mmu.mem_read_bytes(address, T::required_mem())?.as_slice())
    }

    fn read_bounded(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<T, MemoryError> {
        T::decode(self.mmu.mem_read_bytes(address, size)?.as_slice())
    }

    fn read_n(
        &self,
        address: EVAddr,
        n: usize,
    ) -> Result<Vec<T>, MemoryError> {
        self.mmu
            .mem_read_bytes(address, T::required_mem() * n)?
            .chunks(T::required_mem())
            .map(|chunk| try { T::decode(chunk)? })
            .collect()
    }
}
