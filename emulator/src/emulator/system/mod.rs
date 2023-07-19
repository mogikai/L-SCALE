use std::cell::RefCell;

use byteorder::ByteOrder;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;

use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::clock::Clock;
use crate::emulator::system::file_descriptors::fd_map::FileDescriptorMap;
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory::Memory;

pub mod clock;
pub mod default_handles;
pub mod filesystem;
pub mod hardware;
pub mod memory;
pub mod net;
pub mod file_descriptors;

/// This struct contains the global system state and system components like memory and filesystem or the system clock.
/// Also, all currently running tasks are kept here.
#[derive(Debug)]
pub struct System<Endianness: ByteOrder> {
    pub memory: RefCell<Memory<Endianness>>,
    pub filesystem: RefCell<Filesystem>,
    pub clock: RefCell<Clock>,
    pub current_task: RefCell<Option<TaskId>>,
    pub fd_map: RefCell<FileDescriptorMap>,
}

impl<Endianness: ByteOrder> System<Endianness> {
    pub fn new(
        memory: Memory<Endianness>,
        filesystem: Filesystem,
        clock: Clock,
        fd_map: FileDescriptorMap,
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

impl<Endianness: ByteOrder> Restorable for System<Endianness> {
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