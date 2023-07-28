use std::cell::RefCell;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use byteorder::ByteOrder;
use num_enum::FromPrimitive;
use thiserror::Error;

use crate::emulator::abstractions::cpu::{ContextHandle, CPU, CPUError, RegisterX86};
use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::emulator::{Scheduler, SchedulerError, StopReason};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::architecture::x86::X86;
use crate::emulator::platform::linux::kernel::LinuxKernelTask;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::platform::linux::scheduler::syscall::syscall;

mod syscall;

struct SchedulerContext<Endianness: ByteOrder> {
    tasks: BTreeMap<TaskId, LinuxKernelTask<Endianness, X86>>,
    yielded_syscall_rip: Option<u64>,
}

pub struct LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder,
{
    system: Rc<LinuxSystem<Endianness>>,
    context: Rc<RefCell<SchedulerContext<Endianness>>>,
    cpu: Rc<RefCell<CpuT>>,
    cpu_context_handle: ContextHandle,
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
        // TODO: Implement scheduling
        loop {
            self.schedule()?;
            let mut cpu = self.cpu.borrow_mut();
            cpu.restore_context(self.cpu_context_handle)
                .map_err(|_| StopReason::Unknown)?;
            if let Some(rip) = self.context.borrow_mut().yielded_syscall_rip {
                cpu.reg_write(RegisterX86::RIP.into(), rip).unwrap();
            }
            let stop_reason = self.system.current_task.borrow().ok_or(StopReason::Unknown).and_then(|current_task| {
                if let Err(CPUError::Stopped(StopReason::Yield)) = cpu.emulate(None, timeout, instruction_count) {
                    Err(StopReason::Yield)
                } else {
                    Ok(())
                }
            });
            cpu.save_context(self.cpu_context_handle)
                .map_err(|_| StopReason::Unknown)?;
            if let Err(StopReason::Yield) = stop_reason {
                continue;
            } else {
                break stop_reason;
            }
        }.map_err(SchedulerError::from)
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
                yielded_syscall_rip: None,
            })),
            cpu,
            cpu_context_handle,
            phantom: Default::default(),
        };

        sched.setup_initial_task(initial_task, binary_path, binary_args, binary_env)?;

        sched.setup_cpu()?;

        Ok(sched)
    }

    fn schedule(&mut self) -> Result<(), SchedulerError> {
        Ok(())
    }

    fn setup_cpu(&mut self) -> Result<(), SchedulerError> {
        let mut cpu = self.cpu.borrow_mut();

        // TODO: Potentially move this to the clock setup?
        let timestep_system = self.system.clone();
        cpu.add_block_hook(Box::new(move |_, _, instrs| {
            timestep_system.clock.borrow_mut().do_timestep(instrs);
        }));

        let syscall_context = self.context.clone();
        let syscall_system = self.system.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1),
            EVAddr::from(0),
            Box::new(move |cpu: &mut CpuT| {
                let mut context = syscall_context.borrow_mut();
                context.yielded_syscall_rip.take();
                let current_task = syscall_system.current_task.borrow().unwrap();
                let task = context.tasks.get_mut(&current_task).unwrap();
                let result = syscall(cpu, task);
                if let Err(rip) = result {
                    context.yielded_syscall_rip.insert(rip);
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

        cpu.save_context(self.cpu_context_handle)?;
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
        initial_task.load_binary(&mut *cpu, binary_path.as_path(), binary_args, binary_env).map_err(|_| SchedulerError::Unknown)?;
        *self.system.current_task.borrow_mut() = Some(initial_task.task_id());
        let mut context = self.context.borrow_mut();
        context
            .tasks
            .try_insert(initial_task.task_id(), initial_task)
            .map_err(|_| CPUError::TaskExists)?;
        Ok(())
    }
}
