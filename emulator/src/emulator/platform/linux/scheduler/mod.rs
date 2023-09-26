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
use crate::emulator::abstractions::tcp_system::TCPSystem;
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

// TODO: This can probably be generalized to not depend on the system.
//       For example, we could make the System a generic type and also
//       provide a generic syscall callback from the outside.
//       This needs further investigation
pub struct LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a> + Restorable,
    TcpT: TCPSystem,
    Endianness: ByteOrder,
{
    system: LinuxSystem<Endianness, CpuT, TcpT>,
    phantom: PhantomData<&'a CpuT>,
}

impl<'a, CpuT, TcpT, Endianness> Scheduler for LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    TcpT: TCPSystem,
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

impl<'a, CpuT, TcpT, Endianness> LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    TcpT: TCPSystem + 'static,
    Endianness: ByteOrder + 'static,
{
    pub fn new(
        system: LinuxSystem<Endianness, CpuT, TcpT>,
        initial_task: LinuxKernelTask<Endianness, X86, CpuT, TcpT>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>, SchedulerError> {
        let mut sched = LinuxX86Scheduler {
            system,
            phantom: Default::default(),
        };

        sched.setup_initial_task(initial_task, binary_path, binary_args, binary_env)?;

        sched.setup_cpu()?;

        Ok(sched)
    }

    /// Enables trace output for each cpu instruction that is executed.
    /// This is introduces a very large overhead, so only use it when
    /// absolutely necessary. It will also most likely quickly flood
    /// your terminal, so it is recommended to pipe the output into a
    /// file for later analysis.
    pub fn enable_trace(&mut self) -> Result<(), SchedulerError> {
        let trace_memory = self.system.memory.clone();
        let task_manager = self.system.task_manager.clone();
        self.system.cpu.borrow_mut().add_code_hook(
            1.into(),
            0.into(),
            Box::new(move |uc, address, size| {
                let mut memory = trace_memory.borrow_mut();
                trace_hook(uc, &mut memory, task_manager.borrow().current_task().unwrap().task_id(), address, size)
            }),
        )?;
        Ok(())
    }
}
