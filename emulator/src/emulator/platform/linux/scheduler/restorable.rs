use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::scheduler::{LinuxX86Scheduler, SchedulerContext, Task, TaskEntry, TaskState};
use byteorder::ByteOrder;
use std::collections::BTreeMap;
use std::mem::take;

impl<'a, CpuT, Endianness> Restorable for LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + Hooks<'a> + MMU,
    Endianness: 'static + ByteOrder,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.system.snapshot()?;
        self.context.borrow_mut().snapshot()?;
        self.cpu.borrow_mut().snapshot()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.system.restore()?;
        self.context.borrow_mut().restore()?;
        self.cpu.borrow_mut().restore()
    }
}

impl<Endianness: ByteOrder> Restorable for SchedulerContext<Endianness> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.tasks
            .iter_mut()
            .filter_map(TaskEntry::task_mut)
            .try_for_each(Task::snapshot)?;

        self.snapshots.push(SchedulerContextSnapshot {
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

pub(super) struct SchedulerContextSnapshot {
    tasks_end: usize,
    task_map: BTreeMap<TaskId, usize>,
}

impl<Endianness: ByteOrder> Restorable for Task<Endianness> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        // TODO: Does the actual kernel task have any set that needs to be reset?
        //       It probably makes sense to put this state into the system struct
        //       or somewhere similar.
        self.snapshots.push(TaskSnapshot {
            state: self.state,
            yielded_syscall_rip: self.yielded_syscall_rip,
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
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "No Snapshot available"))?;

        self.state = snapshot.state;
        self.yielded_syscall_rip = snapshot.yielded_syscall_rip;
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct TaskSnapshot {
    state: TaskState,
    yielded_syscall_rip: Option<u64>,
}
