use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::{
    GenericError,
    GenericErrorKind,
};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::memory::{
    Memory,
    MemorySnapshot,
};

impl<Endianness: ByteOrder> Restorable for Memory<Endianness> {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(MemorySnapshot {
            layouts: self.layouts.clone(),
            tlbs: self.tlbs.clone(),
            unused_physical_sets: self.unused_physical_sets.clone(),
            physical_end: self.physical_end,
        });
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        if self.snapshots.len() <= 1 {
            Err(GenericError::new(GenericErrorKind::Unknown, "No previous snapshot available"))?;
        }
        self.snapshots.pop().unwrap();
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        let snapshot = self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "No snapshots available"))?;
        self.layouts.clone_from(&snapshot.layouts);
        self.tlbs.clone_from(&snapshot.tlbs);
        self.unused_physical_sets
            .clone_from(&snapshot.unused_physical_sets);
        self.physical_end = snapshot.physical_end;
        Ok(())
    }
}
