use std::marker::PhantomData;

use byteorder::ByteOrder;

use crate::emulator::abstractions::mmu::MMU;

pub mod direct;
pub mod read;
pub mod write;

pub struct MemoryAccessor<'a, Endianness: ByteOrder, MmuT: MMU> {
    pub(super) mmu: &'a mut MmuT,
    phantom_: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder, MmuT: MMU> MemoryAccessor<'_, Endianness, MmuT> {
    pub fn new(mmu: &mut MmuT) -> MemoryAccessor<'_, Endianness, MmuT> {
        MemoryAccessor {
            mmu,
            phantom_: Default::default(),
        }
    }
}
