use std::cell::RefCell;

use byteorder::ByteOrder;

use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::clock::Clock;
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory::Memory;

pub mod clock;
pub mod default_handles;
pub mod filesystem;
pub mod hardware;
pub mod memory;
pub mod net;

/// This struct contains the global system state and system components like memory and filesystem or the system clock.
/// Also, all currently running tasks are kept here.
#[derive(Debug)]
pub struct System<Endianness: ByteOrder> {
    pub memory: RefCell<Memory<Endianness>>,
    pub filesystem: RefCell<Filesystem<Endianness>>,
    pub clock: RefCell<Clock>,
    pub current_task: RefCell<Option<TaskId>>,
}

impl<Endianness: ByteOrder> System<Endianness> {
    pub fn new(
        memory: Memory<Endianness>,
        filesystem: Filesystem<Endianness>,
        clock: Clock,
    ) -> Self {
        Self {
            memory: RefCell::new(memory),
            filesystem: RefCell::new(filesystem),
            clock: RefCell::new(clock),
            current_task: RefCell::new(None),
        }
    }
}
