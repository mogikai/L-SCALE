use crate::emulator::platform::linux::architecture::x86::X86;
use byteorder::ByteOrder;
use rand_pcg::Mcg128Xsl64;

use crate::emulator::platform::linux::fd::{
    FileDescriptorData,
    FD,
};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::kernel::LinuxKernelTask;
use crate::emulator::system::System;

pub mod architecture;
pub mod fd;
pub mod kernel;
pub mod scheduler;
pub mod signalfd;
mod signals;

/// Linux specific system task. Parametrizes the generics with linux specific types
pub(crate) type LinuxSystem<Endianness, CpuT, MmuT, TcpT> = System<
    FD,
    FileDescriptorData,
    TaskId,
    LinuxKernelTask<Endianness, X86, CpuT, MmuT, TcpT>,
    Endianness,
    CpuT,
    MmuT,
    TcpT,
    Mcg128Xsl64,
>;
