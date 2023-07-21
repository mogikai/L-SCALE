use std::cell::RefCell;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::clock::Clock;
use crate::emulator::system::fd_map::{FdId, FileDescriptorMap};
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory::Memory;

pub mod clock;
pub mod default_handles;
pub mod fd_map;
pub mod filesystem;
pub mod hardware;
pub mod memory;

/// This struct contains the global system state and system components like memory and filesystem or the system clock.
/// Also, all currently running tasks are kept here.
#[derive(Debug)]
pub struct System<FD, FDData, TaskId, Endianness>
where
    FD: FdId,
    FDData: Default,
    TaskId: Ord + Copy + Clone + Default,
    Endianness: ByteOrder,
{
    pub memory: RefCell<Memory<Endianness>>,
    pub filesystem: RefCell<Filesystem>,
    pub clock: RefCell<Clock>,
    pub current_task: RefCell<Option<TaskId>>,
    pub fd_map: RefCell<FileDescriptorMap<FD, TaskId, FDData>>,
}

impl<FD, FDData, TaskId, Endianness> System<FD, FDData, TaskId, Endianness>
where
    FD: FdId,
    FDData: Default,
    TaskId: Ord + Copy + Clone + Default,
    Endianness: ByteOrder,
{
    pub fn new(
        memory: Memory<Endianness>,
        filesystem: Filesystem,
        clock: Clock,
        fd_map: FileDescriptorMap<FD, TaskId, FDData>,
    ) -> Self {
        Self {
            memory: RefCell::new(memory),
            filesystem: RefCell::new(filesystem),
            clock: RefCell::new(clock),
            current_task: RefCell::new(None),
            fd_map: RefCell::new(fd_map),
        }
    }
}

impl<FD, FDData, TaskId, Endianness> Restorable for System<FD, FDData, TaskId, Endianness>
where
    FD: FdId,
    FDData: Default,
    TaskId: Ord + Copy + Clone + Default,
    Endianness: ByteOrder,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.memory.get_mut().snapshot()?;
        self.filesystem.get_mut().snapshot()?;
        todo!()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
