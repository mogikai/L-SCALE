use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::kernel_task::KernelTask;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::interface::network::tcp::TCPSystemControl;
use crate::emulator::system::clock::Clock;
use crate::emulator::system::fd_map::{
    FdId,
    FileDescriptorMap,
};
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory::Memory;
use crate::emulator::system::task_manager::TaskManager;
use byteorder::ByteOrder;
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

pub trait SystemControl: Clone {
    type TaskIdT: Ord + Copy + Clone + Default + 'static;
    type KernelTaskT: KernelTask<Self::TaskIdT> + 'static;
    type Endianness: ByteOrder + 'static;
    type CpuT: CPU + MMU + Hooks;
    type TcpT: TCPSystem + TCPSystemControl;

    fn cpu(&self) -> &Rc<RefCell<Self::CpuT>>;
    // TODO: Hide remaining system components behind traits?
    fn memory(&self) -> &Rc<RefCell<Memory<Self::Endianness>>>;
    fn filesystem(&self) -> &Rc<RefCell<Filesystem>>;
    fn clock(&self) -> &Rc<RefCell<Clock>>;
    fn task_manager(&self) -> &Rc<RefCell<TaskManager<Self::TaskIdT, Self::KernelTaskT>>>;
    fn tcp(&self) -> &Rc<RefCell<Self::TcpT>>;
}
