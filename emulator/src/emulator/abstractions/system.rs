use crate::emulator::abstractions::kernel_task::KernelTask;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::interface::network::tcp::TCPSystemControl;
use crate::emulator::system::clock::Clock;
use crate::emulator::system::fd_map::{
    FdId,
    FileDescriptorMap,
};
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory::Memory;
use crate::emulator::system::prng::SeedablePrngMap;
use crate::emulator::system::task_manager::TaskManager;
use byteorder::ByteOrder;
use mogi_system::components::cpu::CPU;
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::components::mmu::MMU;
use mogi_system::components::restore::Restorable;
use rand::{
    Rng,
    SeedableRng,
};
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

pub trait SystemControl: Clone {
    type TaskIdT: Ord + Copy + Clone + Default + 'static;
    type KernelTaskT: KernelTask<Self::TaskIdT> + 'static;
    type Endianness: ByteOrder + 'static;
    type CpuT: CPU + Hooks;
    type MmuT: MMU;
    type TcpT: TCPSystem + TCPSystemControl;
    type FDT: FdId;
    type FDDataT: Default + Clone;
    type PrngT: Rng + SeedableRng + Clone;

    fn cpu(&self) -> &Rc<RefCell<Self::CpuT>>;
    fn mmu(&self) -> &Rc<RefCell<Self::MmuT>>;
    // TODO: Hide remaining system components behind traits?
    fn memory(&self) -> &Rc<RefCell<Memory<Self::Endianness>>>;
    fn filesystem(&self) -> &Rc<RefCell<Filesystem>>;
    fn clock(&self) -> &Rc<RefCell<Clock>>;
    fn prng_map(&self) -> &Rc<RefCell<SeedablePrngMap<Self::PrngT, Self::TaskIdT>>>;
    fn task_manager(&self) -> &Rc<RefCell<TaskManager<Self::TaskIdT, Self::KernelTaskT>>>;
    fn tcp(&self) -> &Rc<RefCell<Self::TcpT>>;
    fn fd_map(&self) -> &Rc<RefCell<FileDescriptorMap<Self::FDT, Self::TaskIdT, Self::FDDataT>>>;
}
