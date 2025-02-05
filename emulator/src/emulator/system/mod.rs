use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

use byteorder::ByteOrder;
use mogi_system::components::mmu::MMU;
use rand::{
    Rng,
    SeedableRng,
};
use rand_pcg::Mcg128Xsl64;

use crate::emulator::abstractions::kernel_task::KernelTask;
use mogi_system::components::cpu::CPU;
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::errors::generic::GenericError;

use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::system::SystemControl;
use crate::emulator::interface::network::tcp::TCPSystemControl;
use crate::emulator::system::clock::Clock;
use crate::emulator::system::fd_map::{
    FdId,
    FileDescriptorMap,
};
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory::Memory;
use crate::emulator::system::stdio::Stdio;
use crate::emulator::system::task_manager::TaskManager;
use mogi_system::components::restore::Restorable;

use self::prng::SeedablePrngMap;

pub mod clock;
pub mod default_handles;
pub mod fd_map;
pub mod filesystem;
pub mod hardware;
pub mod memory;
pub mod prng;
pub mod stdio;
pub mod task_manager;

/// This struct contains the global system state and system components like memory and filesystem or the system clock.
/// Also, all currently running tasks are kept here.
/// The system struct itself may not contain any data, since it is passed by cloning.
/// Any data must be stored behind a pointer. This ensures that individual components can be
/// accessed in a more fine grained manner, instead of having to borrow the entire system at once.
#[derive(Debug)]
pub struct System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT>
where
    FD: FdId,
    FDData: Default + Clone,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId>,
    Endianness: ByteOrder,
    CpuT: CPU,
    MmuT: MMU,
    TcpT: TCPSystem,
    PrngT: Rng + SeedableRng + Clone,
{
    pub cpu: Rc<RefCell<CpuT>>,
    pub mmu: Rc<RefCell<MmuT>>,
    pub memory: Rc<RefCell<Memory<Endianness>>>,
    pub filesystem: Rc<RefCell<Filesystem>>,
    pub clock: Rc<RefCell<Clock>>,
    pub prng_map: Rc<RefCell<SeedablePrngMap<PrngT, TaskId>>>,
    pub fd_map: Rc<RefCell<FileDescriptorMap<FD, TaskId, FDData>>>,
    pub task_manager: Rc<RefCell<TaskManager<TaskId, KernelTaskT>>>,
    pub tcp: Rc<RefCell<TcpT>>,
    pub stdio: Rc<RefCell<Stdio>>,
}

impl<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT> Clone
    for System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT>
where
    FD: FdId,
    FDData: Default + Clone,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId>,
    Endianness: ByteOrder,
    CpuT: CPU,
    MmuT: MMU,
    TcpT: TCPSystem,
    PrngT: Rng + SeedableRng + Clone,
{
    fn clone(&self) -> Self {
        Self {
            cpu: self.cpu.clone(),
            mmu: self.mmu.clone(),
            memory: self.memory.clone(),
            filesystem: self.filesystem.clone(),
            clock: self.clock.clone(),
            prng_map: self.prng_map.clone(),
            fd_map: self.fd_map.clone(),
            task_manager: self.task_manager.clone(),
            tcp: self.tcp.clone(),
            stdio: self.stdio.clone(),
        }
    }
}

impl<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT>
    System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT>
where
    FD: FdId,
    FDData: Default + Clone,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU,
    MmuT: MMU,
    TcpT: TCPSystem,
    PrngT: Rng + SeedableRng + Clone,
{
    pub fn new(
        cpu: Rc<RefCell<CpuT>>,
        mmu: Rc<RefCell<MmuT>>,
        memory: Memory<Endianness>,
        filesystem: Filesystem,
        clock: Clock,
        prng_map: SeedablePrngMap<PrngT, TaskId>,
        fd_map: FileDescriptorMap<FD, TaskId, FDData>,
        task_manager: TaskManager<TaskId, KernelTaskT>,
        tcp: Rc<RefCell<TcpT>>,
        stdio: Rc<RefCell<Stdio>>,
    ) -> Self {
        Self {
            cpu,
            mmu,
            memory: Rc::new(RefCell::new(memory)),
            filesystem: Rc::new(RefCell::new(filesystem)),
            clock: Rc::new(RefCell::new(clock)),
            prng_map: Rc::new(RefCell::new(prng_map)),
            fd_map: Rc::new(RefCell::new(fd_map)),
            task_manager: Rc::new(RefCell::new(task_manager)),
            stdio,
            tcp,
        }
    }
}

impl<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT> SystemControl
    for System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT>
where
    FD: FdId,
    FDData: Default + Clone,
    TaskId: Ord + Copy + Clone + Default + 'static,
    KernelTaskT: KernelTask<TaskId> + 'static,
    Endianness: ByteOrder + 'static,
    CpuT: CPU + Hooks,
    MmuT: MMU,
    TcpT: TCPSystem + TCPSystemControl,
    PrngT: Rng + SeedableRng + Clone,
{
    type TaskIdT = TaskId;
    type KernelTaskT = KernelTaskT;
    type Endianness = Endianness;
    type CpuT = CpuT;
    type MmuT = MmuT;
    type TcpT = TcpT;
    type FDT = FD;
    type FDDataT = FDData;
    type PrngT = PrngT;

    fn cpu(&self) -> &Rc<RefCell<Self::CpuT>> {
        &self.cpu
    }

    fn mmu(&self) -> &Rc<RefCell<Self::MmuT>> {
        &self.mmu
    }

    fn memory(&self) -> &Rc<RefCell<Memory<Self::Endianness>>> {
        &self.memory
    }

    fn filesystem(&self) -> &Rc<RefCell<Filesystem>> {
        &self.filesystem
    }

    fn clock(&self) -> &Rc<RefCell<Clock>> {
        &self.clock
    }

    fn prng_map(&self) -> &Rc<RefCell<SeedablePrngMap<Self::PrngT, Self::TaskIdT>>> {
        &self.prng_map
    }

    fn task_manager(&self) -> &Rc<RefCell<TaskManager<Self::TaskIdT, Self::KernelTaskT>>> {
        &self.task_manager
    }

    fn tcp(&self) -> &Rc<RefCell<Self::TcpT>> {
        &self.tcp
    }

    fn fd_map(&self) -> &Rc<RefCell<FileDescriptorMap<Self::FDT, Self::TaskIdT, Self::FDDataT>>> {
        &self.fd_map
    }

    fn stdio(&self) -> &Rc<RefCell<Stdio>> {
        &self.stdio
    }
}

impl<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT> Restorable
    for System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, MmuT, TcpT, PrngT>
where
    FD: FdId + Debug,
    FDData: Default + Clone + Debug,
    TaskId: Ord + Copy + Clone + Default + Debug,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + Restorable,
    MmuT: MMU + Restorable,
    TcpT: TCPSystem,
    PrngT: Rng + SeedableRng + Clone,
{
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.cpu.borrow_mut().take_snapshot()?;
        self.mmu.borrow_mut().take_snapshot()?;
        self.memory.borrow_mut().take_snapshot()?;
        self.filesystem.borrow_mut().take_snapshot()?;
        self.clock.borrow_mut().take_snapshot()?;
        self.prng_map.borrow_mut().take_snapshot()?;
        self.fd_map.borrow_mut().take_snapshot()?;
        self.task_manager.borrow_mut().take_snapshot()?;
        self.tcp.borrow_mut().take_snapshot()?;
        self.stdio.borrow_mut().take_snapshot()
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        self.cpu
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.mmu
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.memory
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.filesystem
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.clock
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.prng_map
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.fd_map
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.task_manager
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.tcp
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()?;
        self.stdio
            .borrow_mut()
            .discard_current_and_restore_previous_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        self.cpu.borrow_mut().restore_snapshot()?;
        self.memory.borrow_mut().restore_snapshot()?;
        self.memory.borrow_mut().restore_snapshot()?;
        self.filesystem.borrow_mut().restore_snapshot()?;
        self.clock.borrow_mut().restore_snapshot()?;
        self.prng_map.borrow_mut().restore_snapshot()?;
        self.fd_map.borrow_mut().restore_snapshot()?;
        self.task_manager.borrow_mut().restore_snapshot()?;
        self.tcp.borrow_mut().restore_snapshot()?;
        self.stdio.borrow_mut().restore_snapshot()
    }
}
