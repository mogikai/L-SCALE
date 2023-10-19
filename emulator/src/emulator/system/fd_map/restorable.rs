use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::fd_map::{FdId, FileDescriptor, FileDescriptorMap, Snapshot};
use generational_arena::Arena;

impl<D> Restorable for FileDescriptor<D>
where
    D: Restorable + Default,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.data.snapshot()?;
        self.saved_fd_counts.push(self.fd_count);
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.data.restore()?;
        self.fd_count = self
            .saved_fd_counts
            .last()
            .cloned()
            .unwrap_or(self.fd_count);
        Ok(())
    }
}

impl<FD, TaskId, D> Restorable for FileDescriptorMap<FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Restorable,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.files.iter().for_each(|(_, fd)| {
            fd.borrow_mut().snapshot();
        });
        // TODO: How do we handle the files arena with snapshots? We need to truncate anything that
        //       came after a snapshot when restoring. Allow removal of items that are not snapped?
        self.snapshots.push(Snapshot {
            file_descriptors: self.file_descriptors.clone(),
            internal_file_descriptors: self.internal_file_descriptors.clone(),
            free_fds: self.free_fds.clone(),
        });
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        let snapshot = self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "No snapshot available"))?;
        self.files.iter().for_each(|(_, fd)| {
            fd.borrow_mut().restore();
        });
        self.file_descriptors.clone_from(&snapshot.file_descriptors);
        self.internal_file_descriptors
            .clone_from(&snapshot.internal_file_descriptors);
        self.free_fds.clone_from(&snapshot.free_fds);
        Ok(())
    }
}
