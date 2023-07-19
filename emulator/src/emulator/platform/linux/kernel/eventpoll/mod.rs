use std::collections::BTreeMap;
use bitflags::{bitflags, Flags};

use byteorder::ByteOrder;

use fuzzyemu_derive::{Decode, Encode, RequiredMem};

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::mmu::{MemoryError, MMU};
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::{LinuxKernelError, LinuxKernelTask, SyscallResult};
use crate::emulator::platform::linux::kernel::FileDescriptor;
use crate::emulator::system::file_descriptors::fd_map::FD;
use crate::emulator::system::memory::rw_interface::MemoryRead;

impl<Endianness: ByteOrder, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    pub fn epoll_create(
        &mut self,
        _: &mut impl CPU,
        size: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn epoll_wait(
        &mut self,
        _: &mut impl CPU,
        epfd: u64,
        events: EVAddr, // epoll_event*
        maxevents: u64,
        timeout: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn epoll_ctl(
        &mut self,
        mmu: &mut impl MMU,
        epfd: FD,
        op: u64,
        fd: FD,
        event: EVAddr, // epoll_event*
    ) -> SyscallResult {
        // TODO: The poll and select syscalls require files to have a poll/select function that can be called by
        //       the epoll/select handler to query whether or not data is available for read or write ops
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut memory = self.system.memory.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        let mut epfd = file_descriptor_map.get_mut(&self.task_id, &epfd).ok_or(LinuxKernelError::EBADF)?;
        let epoll = epfd.data.epoll.as_mut().ok_or(LinuxKernelError::EPERM)?;

        let event: epoll_event = memory.read(event)?;
        epoll.interest_list.insert(fd, event);

        error!("TODO: Implement epoll_ctl. Need to add fds to interest list");
        Ok(0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn epoll_pwait(
        &mut self,
        _: &mut impl CPU,
        epfd: u64,
        events: EVAddr, // epoll_event*
        maxevents: u64,
        timeout: u64,
        sigmask: EVAddr, // sigset_t*
        sigsetsize: u64,
    ) -> SyscallResult {
        // TODO: We need to possibly yield to the scheduler here. So we would need to stop emulation and return
        //       control to outside of the syscall handler so we can perform a context switch.
        //       Another possibility would be to simply not yield here and employ some kind of time slice
        //       scheduling, where each thread / process is run for X iterations.
        todo!()
    }

    pub fn epoll_create1(
        &mut self,
        _: &mut impl CPU,
        flags: u64,
    ) -> SyscallResult {
        let mut fd = FileDescriptor::new(Box::new(EpollOps::new()));
        fd.data.epoll = Some(EpollData::new());
        Ok(self.system.fd_map.borrow_mut().insert(&self.task_id, fd)?.into())
    }
}

#[derive(Debug)]
pub struct EpollData {
    interest_list: BTreeMap<FD, epoll_event>,
}

impl EpollData {
    fn new() -> Self { EpollData { interest_list: Default::default() } }
}

#[derive(Debug, Decode, Encode, RequiredMem)]
#[repr(C)]
struct epoll_event {
    events: Events,
    data: u64,
}

bitflags! {
    #[derive(Debug, RequiredMem)]
    #[repr(C)]
    pub struct Events: u32 {
        const Read              = 0x0001;
        const Priority          = 0x0002;
        const Write             = 0x0004;
        const Error             = 0x0008;
        const HangUp            = 0x0010;
        const Invalid           = 0x0020;
        const ReadNormal        = 0x0040;
        const ReadOutOfBand     = 0x0080;
        const WriteNormal       = 0x0100;
        const WriteOutOfBand    = 0x0200;
        const ReadHangUp        = 0x2000;
    }
}

impl Decode for Events {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        Self::from_bits(Endianness::read_u32(val)).ok_or(MemoryError::ConversionError)
    }
}

impl Encode for Events {
    fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
        self.bits().encode::<Endianness>(buffer)
    }
}


#[derive(Debug)]
pub struct EpollOps {}

impl EpollOps {
    fn new() -> Self { Self {} }
}

impl FileDescriptorOps for EpollOps {}
