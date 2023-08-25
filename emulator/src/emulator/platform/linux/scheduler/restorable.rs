use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::scheduler::{LinuxX86Scheduler, SchedulerContext, Task, TaskEntry};
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
        self.cpu.borrow_mut().snapshot()?;
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}

impl<Endianness: ByteOrder> Restorable for SchedulerContext<Endianness> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        let previous_end = self.snapshots.last().map_or(0, |s| s.tasks_size);
        // Snapshot all tasks that are not in any previous snapshots
        self.tasks
            .iter_mut()
            .skip(previous_end)
            .filter_map(TaskEntry::task_mut)
            .try_for_each(Task::snapshot)?;
        self.deleted_tasks.iter().for_each(|index| {
            self.tasks.get_mut(*index).unwrap().delete();
        });
        self.snapshots.push(SchedulerContextSnapshot {
            tasks_size: self.tasks.len(),
            deleted_tasks: take(&mut self.deleted_tasks),
            task_map: self.task_map.clone(),
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

pub(super) struct SchedulerContextSnapshot {
    tasks_size: usize,
    deleted_tasks: Vec<usize>,
    task_map: BTreeMap<TaskId, usize>,
}

impl<Endianness: ByteOrder> Restorable for Task<Endianness> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        // TODO: Does the actual kernel task have any set that needs to be reset?
        //       It probably makes sense to put this state into the system struct
        //       or somewhere similar.
        todo!()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
