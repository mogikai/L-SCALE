use std::marker::PhantomData;
use std::ops::Try;
use std::path::PathBuf;
use std::time::Duration;

use byteorder::ByteOrder;
use mogi_system::components::cpu::CPU;
use mogi_system::components::mmu::MMU;
use mogi_system::errors::SchedulerError;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::emulator::Scheduler;
use mogi_system::components::cpu::hooks::Hooks;

use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::architecture::x86::X86;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::kernel::LinuxKernelTask;
use crate::emulator::platform::linux::scheduler::syscall::syscall;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::memory::Memory;
use mogi_system::components::restore::Restorable;

mod main_loop;
mod restorable;
mod setup;
mod syscall;

// TODO: This can probably be generalized to not depend on the system.
//       For example, we could make the System a generic type and also
//       provide a generic syscall callback from the outside.
//       This needs further investigation
#[derive(Debug)]
pub struct LinuxX86Scheduler<'a, CpuT, MmuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + Restorable,
    MmuT: MMU,
    TcpT: TCPSystem,
    Endianness: ByteOrder,
{
    system: LinuxSystem<Endianness, CpuT, MmuT, TcpT>,
    phantom: PhantomData<&'a CpuT>,
}

impl<'a, CpuT, MmuT, TcpT, Endianness> Scheduler for LinuxX86Scheduler<'a, CpuT, MmuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + Restorable,
    MmuT: MMU,
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

impl<'a, CpuT, MmuT, TcpT, Endianness> LinuxX86Scheduler<'a, CpuT, MmuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + Restorable + 'static,
    MmuT: MMU + 'static,
    TcpT: TCPSystem + 'static,
    Endianness: ByteOrder + 'static,
{
    pub fn new(
        system: LinuxSystem<Endianness, CpuT, MmuT, TcpT>,
        initial_task: LinuxKernelTask<Endianness, X86, CpuT, MmuT, TcpT>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<LinuxX86Scheduler<'a, CpuT, MmuT, TcpT, Endianness>, SchedulerError> {
        let mut sched = LinuxX86Scheduler {
            system,
            phantom: Default::default(),
        };

        sched.setup_initial_task(initial_task, binary_path, binary_args, binary_env)?;

        sched.setup_cpu()?;

        Ok(sched)
    }
}
