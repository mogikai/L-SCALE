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
use crate::emulator::platform::linux::scheduler::syscall::syscall;
use crate::emulator::platform::linux::scheduler::trace::trace_hook;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::memory::Memory;

mod main_loop;
mod restorable;
mod setup;
mod syscall;
mod trace;

enum TaskState {
    Runnable,
    Yielded,
}

struct Task<Endianness: ByteOrder> {
    kernel: LinuxKernelTask<Endianness, X86>,
    state: TaskState,
    cpu_context_handle: ContextHandle,
    yielded_syscall_rip: Option<u64>,
}

struct SchedulerContext<Endianness: ByteOrder> {
    tasks: BTreeMap<TaskId, Task<Endianness>>,
    current_task: Rc<RefCell<Option<Task<Endianness>>>>,
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

    fn try_for_each_task<F: FnMut(&LinuxKernelTask<Endianness, X86>) -> R, R: Try<Output = ()>>(
        &self,
        callback: F,
    ) -> R {
        self.context
            .borrow()
            .tasks
            .values()
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
            context: Rc::new(RefCell::new(SchedulerContext {
                tasks: Default::default(),
                current_task: Default::default(),
            })),
            cpu,
            phantom: Default::default(),
        };

        sched.setup_initial_task(initial_task, binary_path, binary_args, binary_env)?;

        sched.setup_cpu()?;

        Ok(sched)
    }

    pub fn enable_trace(&mut self) -> Result<(), SchedulerError> {
        let trace_memory = self.system.memory.clone();
        let current_task = self.context.borrow_mut().current_task.clone();
        self.cpu.borrow_mut().add_code_hook(
            1.into(),
            0.into(),
            Box::new(move |uc, address, size| {
                let mut memory = trace_memory.borrow_mut();
                trace_hook(uc, &mut memory, current_task.borrow().as_ref().unwrap().kernel.task_id(), address, size)
            }),
        )?;
        Ok(())
    }
}
