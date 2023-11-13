use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::kernel_task::KernelTask;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::clock::Clock;
use crate::emulator::system::fd_map::{
    FdId,
    FileDescriptorMap,
};
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory::Memory;
use crate::emulator::system::task_manager::TaskManager;

pub mod clock;
pub mod default_handles;
pub mod fd_map;
pub mod filesystem;
pub mod hardware;
pub mod memory;
pub mod task_manager;

/// This struct contains the global system state and system components like memory and filesystem or the system clock.
/// Also, all currently running tasks are kept here.
/// The system struct itself may not contain any data, since it is passed by cloning.
/// Any data must be stored behind a pointer. This ensures that individual components can be
/// accessed in a more fine grained manner, instead of having to borrow the entire system at once.
#[derive(Debug)]
pub struct System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    pub cpu: Rc<RefCell<CpuT>>,
    pub memory: Rc<RefCell<Memory<Endianness>>>,
    pub filesystem: Rc<RefCell<Filesystem>>,
    pub clock: Rc<RefCell<Clock>>,
    pub fd_map: Rc<RefCell<FileDescriptorMap<FD, TaskId, FDData>>>,
    pub task_manager: Rc<RefCell<TaskManager<TaskId, KernelTaskT>>>,
    pub tcp: Rc<RefCell<TcpT>>,
}

impl<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT> Clone
    for System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    fn clone(&self) -> Self {
        Self {
            cpu: self.cpu.clone(),
            memory: self.memory.clone(),
            filesystem: self.filesystem.clone(),
            clock: self.clock.clone(),
            fd_map: self.fd_map.clone(),
            task_manager: self.task_manager.clone(),
            tcp: self.tcp.clone(),
        }
    }
}

impl<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
    System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    pub fn new(
        cpu: Rc<RefCell<CpuT>>,
        memory: Memory<Endianness>,
        filesystem: Filesystem,
        clock: Clock,
        fd_map: FileDescriptorMap<FD, TaskId, FDData>,
        task_manager: TaskManager<TaskId, KernelTaskT>,
        tcp: Rc<RefCell<TcpT>>,
    ) -> Self {
        Self {
            cpu,
            memory: Rc::new(RefCell::new(memory)),
            filesystem: Rc::new(RefCell::new(filesystem)),
            clock: Rc::new(RefCell::new(clock)),
            fd_map: Rc::new(RefCell::new(fd_map)),
            task_manager: Rc::new(RefCell::new(task_manager)),
            tcp,
        }
    }
}

impl<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT> Restorable
    for System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.cpu.borrow_mut().snapshot()?;
        self.memory.borrow_mut().snapshot()?;
        self.filesystem.borrow_mut().snapshot()?;
        self.clock.borrow_mut().snapshot()?;
        self.fd_map.borrow_mut().snapshot()?;
        self.task_manager.borrow_mut().snapshot()?;
        self.tcp.borrow_mut().snapshot()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.cpu.borrow_mut().restore()?;
        self.memory.borrow_mut().restore()?;
        self.filesystem.borrow_mut().restore()?;
        self.clock.borrow_mut().restore()?;
        self.fd_map.borrow_mut().restore()?;
        self.task_manager.borrow_mut().restore()?;
        self.tcp.borrow_mut().restore()
    }
}
