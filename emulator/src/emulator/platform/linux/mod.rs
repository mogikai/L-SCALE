use byteorder::ByteOrder;

use crate::emulator::platform::linux::fd::{FileDescriptorData, FD};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::System;

pub mod architecture;
pub mod scheduler;
pub mod fd;
pub mod kernel;
pub mod signalfd;
mod signals;

/// Linux specific system task. Parametrizes the generics with linux specific types
pub(crate) type LinuxSystem<Endianness> = System<FD, FileDescriptorData, TaskId, Endianness>;
