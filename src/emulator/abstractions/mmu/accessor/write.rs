use byteorder::ByteOrder;
use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::mmu::accessor::MemoryAccessor;
use crate::emulator::abstractions::mmu::{Encode, MemoryError, MemoryWrite, MMU};

impl<Endianness: ByteOrder, MmuT: MMU, T: Encode<Endianness>> MemoryWrite<T, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError> {
        self.mmu.mem_write_bytes(address, data.encode().as_slice())
    }
}