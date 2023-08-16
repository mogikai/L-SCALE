use std::cell::RefCell;
use std::rc::Rc;

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
/// The system struct itself may not contain any data, since it is passed by cloning.
/// Any data must be stored behind a pointer. This ensures that individual components can be
/// accessed in a more fine grained manner, instead of having to borrow the entire system at once.
#[derive(Debug)]
pub struct System<FD, FDData, TaskId, Endianness>
where
    FD: FdId,
    FDData: Default,
    TaskId: Ord + Copy + Clone + Default,
    Endianness: ByteOrder,
{
    pub memory: Rc<RefCell<Memory<Endianness>>>,
    pub filesystem: Rc<RefCell<Filesystem>>,
    pub clock: Rc<RefCell<Clock>>,
    pub fd_map: Rc<RefCell<FileDescriptorMap<FD, TaskId, FDData>>>,
}

impl<FD, FDData, TaskId, Endianness> Clone for System<FD, FDData, TaskId, Endianness>
where
    FD: FdId,
    FDData: Default,
    TaskId: Ord + Copy + Clone + Default,
    Endianness: ByteOrder,
{
    fn clone(&self) -> Self {
        Self {
            memory: self.memory.clone(),
            filesystem: self.filesystem.clone(),
            clock: self.clock.clone(),
            fd_map: self.fd_map.clone(),
        }
    }
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
            memory: Rc::new(RefCell::new(memory)),
            filesystem: Rc::new(RefCell::new(filesystem)),
            clock: Rc::new(RefCell::new(clock)),
            fd_map: Rc::new(RefCell::new(fd_map)),
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
        self.memory.borrow_mut().snapshot()?;
        self.filesystem.borrow_mut().snapshot()?;
        self.clock.borrow_mut().snapshot()?;
        todo!();
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
