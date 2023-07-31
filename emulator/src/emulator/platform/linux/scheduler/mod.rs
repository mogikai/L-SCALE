use std::cell::RefCell;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::{CPUError, ContextHandle, RegisterX86, CPU};
use crate::emulator::abstractions::emulator::{Scheduler, SchedulerError, StopReason};
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::architecture::x86::X86;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::kernel::LinuxKernelTask;
use crate::emulator::platform::linux::scheduler::syscall::syscall;
use crate::emulator::platform::linux::LinuxSystem;

mod syscall;

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
    current_task: Option<Task<Endianness>>,
}

pub struct LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder,
{
    system: Rc<LinuxSystem<Endianness>>,
    context: Rc<RefCell<SchedulerContext<Endianness>>>,
    cpu: Rc<RefCell<CpuT>>,
    phantom: PhantomData<&'a CpuT>,
}

impl<'a, CpuT, Endianness> Scheduler for LinuxX86Scheduler<'a, CpuT, Endianness>
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
}

impl<'a, CpuT, Endianness> LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder + 'static,
{
    pub fn new(
        system: Rc<LinuxSystem<Endianness>>,
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
                current_task: None,
            })),
            cpu,
            phantom: Default::default(),
        };

        sched.setup_initial_task(initial_task, binary_path, binary_args, binary_env)?;

        sched.setup_cpu()?;

        Ok(sched)
    }

    fn scheduler_loop(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError> {
        loop {
            let task_id = self.schedule()?;

            self.pre_task(&task_id)?;
            let stop_reason = self.process_task(timeout, instruction_count)?;
            self.post_task(stop_reason)?;
        }
    }

    fn schedule(&mut self) -> Result<TaskId, SchedulerError> {
        // TODO: Implement actual scheduling. Potentially delegate to fuzzer.
        self.context
            .borrow_mut()
            .tasks
            .iter()
            .find(|(id, task)| matches!(task.state, TaskState::Runnable))
            .map(|(id, _)| *id)
            .ok_or(SchedulerError::NoRunnableTasks)
    }

    fn pre_task(
        &mut self,
        task_id: &TaskId,
    ) -> Result<(), SchedulerError> {
        let mut context = self.context.borrow_mut();
        let mut cpu = self.cpu.borrow_mut();

        let task = context
            .tasks
            .remove(task_id)
            .expect("Scheduled task must exist");

        cpu.restore_context(task.cpu_context_handle)
            .map_err(|_| StopReason::Unknown)?;

        if let Some(rip) = task.yielded_syscall_rip {
            cpu.reg_write(RegisterX86::RIP.into(), rip).unwrap();
        }
        context.current_task.insert(task);
        Ok(())
    }

    fn process_task(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<Option<StopReason>, CPUError> {
        let result = self
            .cpu
            .borrow_mut()
            .emulate(None, timeout, instruction_count);
        match result {
            Err(CPUError::Stopped(reason)) => Ok(Some(reason)),
            Err(e) => Err(e),
            _ => Ok(None),
        }
    }

    fn post_task(
        &mut self,
        stop_reason: Option<StopReason>,
    ) -> Result<(), SchedulerError> {
        // Save all the context after we are done executing the scheduled task
        let mut context = self.context.borrow_mut();
        let mut cpu = self.cpu.borrow_mut();

        let mut task = context.current_task.take().unwrap();
        cpu.save_context(task.cpu_context_handle)
            .map_err(|_| StopReason::Unknown)?;

        let error = match stop_reason {
            None => Ok(()),
            Some(StopReason::Yield) => {
                task.state = TaskState::Yielded;
                Ok(())
            }
            Some(StopReason::Killed) => {
                // TODO: Remove Task from list of runnable tasks
                Ok(())
            }
            Some(reason) => Err(reason),
        };

        context.tasks.insert(task.kernel.task_id(), task);

        Ok(error?)
    }

    fn setup_cpu(&mut self) -> Result<(), SchedulerError> {
        let mut cpu = self.cpu.borrow_mut();

        // TODO: Potentially move this to the clock setup?
        let timestep_system = self.system.clone();
        cpu.add_block_hook(Box::new(move |_, _, instrs| {
            timestep_system.clock.borrow_mut().do_timestep(instrs);
        }));

        let syscall_context = self.context.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1),
            EVAddr::from(0),
            Box::new(move |cpu: &mut CpuT| {
                let mut context = syscall_context.borrow_mut();
                let task = context
                    .current_task
                    .as_mut()
                    .expect("The current task must be set by the scheduler");
                task.yielded_syscall_rip.take();
                let result = syscall(cpu, &mut task.kernel);
                if let Err(rip) = result {
                    task.yielded_syscall_rip.insert(rip);
                }
            }),
        );

        cpu.add_unmapped_mem_hook(
            1.into(),
            0.into(),
            Box::new(move |_, action, addr, size, _| {
                trace!("{:?} on {:#X} with size {} ", action, addr, size);
                false
            }),
        );

        Ok(())
    }

    fn setup_initial_task(
        &mut self,
        mut initial_task: LinuxKernelTask<Endianness, X86>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), SchedulerError> {
        initial_task.add_stdio_handles();

        let mut cpu = self.cpu.borrow_mut();
        // TODO: Do more specific error mapping
        initial_task
            .load_binary(&mut *cpu, binary_path.as_path(), binary_args, binary_env)
            .map_err(|_| SchedulerError::Unknown)?;
        *self.system.current_task.borrow_mut() = Some(initial_task.task_id());
        let mut context = self.context.borrow_mut();
        let task = Task {
            kernel: initial_task,
            state: TaskState::Runnable,
            cpu_context_handle: cpu.create_context()?,
            yielded_syscall_rip: None,
        };
        cpu.save_context(task.cpu_context_handle)?;
        context
            .tasks
            .try_insert(task.kernel.task_id(), task)
            .map_err(|_| CPUError::TaskExists)?;
        Ok(())
    }
}
