use super::*;
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::restore::Restorable;
use std::collections::BTreeMap;

impl<TaskId, T> Restorable for TaskManager<TaskId, T>
where
    TaskId: Ord + Copy + Clone + Default,
    T: KernelTask<TaskId> + Debug,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.tasks
            .iter_mut()
            .filter_map(TaskEntry::task_mut)
            .try_for_each(Task::snapshot)?;

        self.snapshots.push(TaskManagerSnapshot {
            tasks_end: self.tasks.len(),
            task_map: self.task_map.clone(),
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

        // Discard all tasks that were created after the snapshot.
        self.tasks.truncate(snapshot.tasks_end);

        self.tasks
            .iter_mut()
            .filter_map(TaskEntry::task_mut)
            .try_for_each(Task::restore)?;

        self.task_map.clone_from(&snapshot.task_map);
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct TaskManagerSnapshot<TaskId>
where
    TaskId: Ord + Copy + Clone + Default,
{
    tasks_end: usize,
    task_map: BTreeMap<TaskId, usize>,
}

impl<TaskId, T: KernelTask<TaskId>> Restorable for Task<TaskId, T> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        // TODO: Does the actual kernel task have any set that needs to be reset?
        //       It probably makes sense to put this state into the system struct
        //       or somewhere similar.
        self.snapshots.push(TaskSnapshot { state: self.state });
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        let snapshot = self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "No Snapshot available"))?;

        self.state = snapshot.state;
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct TaskSnapshot {
    state: TaskState,
}
