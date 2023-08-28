use std::cell::RefCell;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::ops::Try;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::{CPUError, ContextHandle, RegisterX86, CPU};
use crate::emulator::abstractions::emulator::{Scheduler, SchedulerError, StopReason};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::architecture::x86::X86;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::kernel::LinuxKernelTask;
use crate::emulator::platform::linux::scheduler::restorable::{SchedulerContextSnapshot, TaskSnapshot};
use crate::emulator::platform::linux::scheduler::syscall::syscall;
use crate::emulator::platform::linux::scheduler::trace::trace_hook;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::memory::Memory;

mod main_loop;
mod restorable;
mod setup;
mod syscall;
mod trace;

#[derive(Debug, Copy, Clone)]
enum TaskState {
    Runnable,
    Yielded,
}

#[derive(Debug)]
struct Task<Endianness: ByteOrder> {
    kernel: LinuxKernelTask<Endianness, X86>,
    state: TaskState,
    cpu_context_handle: ContextHandle,
    yielded_syscall_rip: Option<u64>,
    snapshots: Vec<TaskSnapshot>,
}

#[derive(Debug)]
enum TaskEntry<Endianness: ByteOrder> {
    Deleted(Option<Task<Endianness>>),
    Task(Option<Task<Endianness>>),
}

impl<Endianness: ByteOrder> From<Task<Endianness>> for TaskEntry<Endianness> {
    fn from(value: Task<Endianness>) -> Self {
        Self::Task(Some(value))
    }
}

impl<Endianness: ByteOrder> TaskEntry<Endianness> {
    /// Transforms the TaskEntry into a deleted entry. The contained Task is unaffected by this.
    /// The main purpose of this is to signify that the task was deleted, but is still kept
    /// in order to be able to restore a previous snapshot.
    fn delete(&mut self) {
        if let TaskEntry::Task(t) = self {
            *self = TaskEntry::Deleted(Some(t.take().unwrap()))
        }
    }

    fn restore(&mut self) {
        if let TaskEntry::Deleted(t) = self {
            *self = TaskEntry::Task(Some(t.take().unwrap()))
        }
    }

    fn task(&self) -> Option<&Task<Endianness>> {
        match self {
            TaskEntry::Deleted(t) => None,
            TaskEntry::Task(t) => t.as_ref(),
        }
    }

    fn task_mut(&mut self) -> Option<&mut Task<Endianness>> {
        match self {
            TaskEntry::Deleted(t) => None,
            TaskEntry::Task(t) => t.as_mut(),
        }
    }
}

#[derive(Default)]
struct SchedulerContext<Endianness: ByteOrder> {
    /// The tasks are stored in a simple vector. Only elements that are newer than the latest
    /// snapshot can be removed from this vector. When a snapshot occurs, each entry in the
    /// vector is also snapshot.
    tasks: Vec<TaskEntry<Endianness>>,
    // We want to save which tasks we deleted that are contained in some snapshot. We will have
    // to restore these if we decide to snapshot the current state.
    deleted_tasks: Vec<usize>,
    /// This is simply a mapping of `TaskId` to an index into the tasks vector.
    /// This makes it possible to do an expensive lookup once, and then simply access by index.
    task_map: BTreeMap<TaskId, usize>,
    /// The current task is an index into the tasks vector
    current_task: Rc<RefCell<Option<usize>>>,
    snapshots: Vec<SchedulerContextSnapshot>,
}

impl<Endianness: ByteOrder> SchedulerContext<Endianness> {
    fn current_task(&self) -> Option<&Task<Endianness>> {
        let current_task = (*self.current_task.borrow())?;
        self.tasks.get(current_task).and_then(TaskEntry::task)
    }

    fn current_task_mut(&mut self) -> Option<&mut Task<Endianness>> {
        let current_task = (*self.current_task.borrow())?;
        self.tasks
            .get_mut(current_task)
            .and_then(TaskEntry::task_mut)
    }
}

pub struct LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a> + Restorable,
    Endianness: ByteOrder,
{
    system: LinuxSystem<Endianness>,
    context: Rc<RefCell<SchedulerContext<Endianness>>>,
    cpu: Rc<RefCell<CpuT>>,
    phantom: PhantomData<&'a CpuT>,
}

impl<'a, CpuT, Endianness> Scheduler<LinuxKernelTask<Endianness, X86>> for LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder + 'static,
{
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError> {
        self.scheduler_loop(timeout, instruction_count)
    }

    fn try_for_each_task<F, R>(
        &self,
        callback: F,
    ) -> R
    where
        F: FnMut(&LinuxKernelTask<Endianness, X86>) -> R,
        R: Try<Output = ()>,
    {
        self.context
            .borrow()
            .tasks
            .iter()
            .filter_map(TaskEntry::task)
            .map(|t| &t.kernel)
            .try_for_each(callback)
    }
}

impl<'a, CpuT, Endianness> LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder + 'static,
{
    pub fn new(
        system: LinuxSystem<Endianness>,
        initial_task: LinuxKernelTask<Endianness, X86>,
        cpu: Rc<RefCell<CpuT>>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<LinuxX86Scheduler<'a, CpuT, Endianness>, SchedulerError> {
        let cpu_context_handle = cpu.borrow_mut().create_context()?;
        let mut sched = LinuxX86Scheduler {
            system,
            context: Default::default(),
            cpu,
            phantom: Default::default(),
        };

        sched.setup_initial_task(initial_task, binary_path, binary_args, binary_env)?;

        sched.setup_cpu()?;

        Ok(sched)
    }

    pub fn enable_trace(&mut self) -> Result<(), SchedulerError> {
        let trace_memory = self.system.memory.clone();
        let sched_context = self.context.clone();
        self.cpu.borrow_mut().add_code_hook(
            1.into(),
            0.into(),
            Box::new(move |uc, address, size| {
                let mut memory = trace_memory.borrow_mut();
                trace_hook(
                    uc,
                    &mut memory,
                    sched_context
                        .borrow()
                        .current_task()
                        .unwrap()
                        .kernel
                        .task_id(),
                    address,
                    size,
                )
            }),
        )?;
        Ok(())
    }
}
