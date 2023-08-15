use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::memory::{Memory, MemorySnapshot};

impl<Endianness: ByteOrder> Restorable for Memory<Endianness> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(MemorySnapshot {
            layouts: self.layouts.clone(),
            tlbs: self.tlbs.clone(),
            unused_physical_sets: self.unused_physical_sets.clone(),
            physical_end: self.physical_end,
        });
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
