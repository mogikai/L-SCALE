use crate::emulator::platform::linux::architecture::x86::X86;
use byteorder::ByteOrder;

use crate::emulator::platform::linux::fd::{FileDescriptorData, FD};
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
pub(crate) type LinuxSystem<Endianness, CpuT> =
    System<FD, FileDescriptorData, TaskId, LinuxKernelTask<Endianness, X86, CpuT>, Endianness, CpuT>;
