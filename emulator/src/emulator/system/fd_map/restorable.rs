use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::fd_map::{FdId, FileDescriptorMap, Snapshot};
use generational_arena::Arena;

impl<FD, TaskId, D> Restorable for FileDescriptorMap<FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Restorable,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.files.iter().for_each(|(_, fd)| {
            fd.borrow_mut().data.snapshot();
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
        todo!()
    }
}
