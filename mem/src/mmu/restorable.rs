use crate::mmu::{
    MemoryContext,
    MogiMMU,
};
use crate::physical::PhysicalMemory;
use mogi_system::components::restore::Restorable;
use mogi_system::errors::generic::GenericError;

pub(super) struct Snapshot {
    memory_context: MemoryContext,
    physical_memory: PhysicalMemory,
}

impl Restorable for MogiMMU {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(Snapshot {
            memory_context: self.memory_context.clone(),
            physical_memory: self.physical_memory.clone(),
        });
        self.flush();
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.pop();
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        let snapshot = self.snapshots.last().ok_or(GenericError::unknown())?;
        self.memory_context.clone_from(&snapshot.memory_context);
        self.physical_memory.clone_from(&snapshot.physical_memory);
        Ok(())
    }
}
