use crate::emulator::system::fd_map::{
    FdId,
    FileDescriptorMap,
    Snapshot,
};
use mogi_system::components::restore::Restorable;
use mogi_system::errors::generic::{
    GenericError,
    GenericErrorKind,
};
use std::fmt::Debug;

impl<FD, TaskId, D> Restorable for FileDescriptorMap<FD, TaskId, D>
where
    FD: FdId + Debug,
    TaskId: Ord + Copy + Clone + Default + Debug,
    D: Default + Clone + Debug,
{
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        // TODO: How do we handle the files arena with snapshots? We need to truncate anything that
        //       came after a snapshot when restoring. Allow removal of items that are not snapped?
        self.snapshots.push(Snapshot {
            files: self.files.clone(),
            file_descriptors: self.file_descriptors.clone(),
            internal_file_descriptors: self.internal_file_descriptors.clone(),
            free_fds: self.free_fds.clone(),
            cwds: self.cwds.clone(),
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
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "No snapshot available"))?;
        self.files.clone_from(&snapshot.files);
        self.file_descriptors.clone_from(&snapshot.file_descriptors);
        self.internal_file_descriptors
            .clone_from(&snapshot.internal_file_descriptors);
        self.free_fds.clone_from(&snapshot.free_fds);
        self.cwds.clone_from(&snapshot.cwds);
        Ok(())
    }
}
