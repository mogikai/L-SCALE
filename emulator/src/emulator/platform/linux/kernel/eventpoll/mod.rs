use std::collections::BTreeMap;

use bitflags::{bitflags, Flags};
use byteorder::ByteOrder;
use num_enum::TryFromPrimitive;

use fuzzyemu_derive::{Decode, Encode, RequiredMem};

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};
use crate::emulator::abstractions::mmu::{MemoryError, MMU};
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::filedescriptor::FileDescriptorData;
use crate::emulator::platform::linux::kernel::{FileDescriptor, LinuxKernelError, LinuxKernelTask, SyscallResult};
use crate::emulator::system::file_descriptors::fd_map::FD;
use crate::emulator::system::memory::rw_interface::MemoryRead;
use crate::emulator::system::memory::MemoryAccessor;

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
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut memory = self.system.memory.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);

        let op = EPOLL_OPCODES::try_from_primitive(op).map_err(|_| LinuxKernelError::EINVAL)?;

        if epfd == fd {
            Err(LinuxKernelError::EINVAL)?
        }

        // Check if fd is valid by fetching it
        let mut file_descriptor = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let file_descriptor = &mut *file_descriptor;
        let mut epfd = file_descriptor_map
            .get_mut(&self.task_id, &epfd)
            .ok_or(LinuxKernelError::EBADF)?;

        // Is this an epoll fd?
        let epoll = epfd.data.epoll.as_mut().ok_or(LinuxKernelError::EINVAL)?;

        // Can fd be polled?
        if !file_descriptor.ops.supports_poll(&mut file_descriptor.data) {
            Err(LinuxKernelError::EINVAL)?
        }

        match op {
            EPOLL_OPCODES::EPOLL_CTL_ADD => self.epoll_ctl_add(&memory, epoll, &fd, file_descriptor, event),
            EPOLL_OPCODES::EPOLL_CTL_DEL => self.epoll_ctl_del(epoll, &fd),
            EPOLL_OPCODES::EPOLL_CTL_MOD => self.epoll_ctl_mod(&memory, epoll, &fd, event),
        }
    }

    fn epoll_ctl_add(
        &self,
        memory: &MemoryAccessor<Endianness, impl MMU + Sized>,
        epoll: &mut EpollData,
        fd: &FD,
        file_descriptor: &mut FileDescriptor,
        event: EVAddr,
    ) -> SyscallResult {
        // TODO: Do loop check (see https://man7.org/linux/man-pages/man2/epoll_ctl.2.html)
        let event: epoll_event = memory.read(event)?;

        if event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) && file_descriptor.data.epoll.is_some() {
            Err(LinuxKernelError::EINVAL)?
        }

        epoll
            .interest_list
            .try_insert(*fd, event)
            .map_err(|_| LinuxKernelError::EEXIST)?;
        Ok(0)
    }

    fn epoll_ctl_del(
        &self,
        epoll: &mut EpollData,
        fd: &FD,
    ) -> SyscallResult {
        epoll
            .interest_list
            .remove(&fd)
            .ok_or(LinuxKernelError::ENOENT)?;
        Ok(0)
    }

    fn epoll_ctl_mod(
        &self,
        memory: &MemoryAccessor<Endianness, impl MMU + Sized>,
        epoll: &mut EpollData,
        fd: &FD,
        event: EVAddr,
    ) -> SyscallResult {
        let event: epoll_event = memory.read(event)?;

        if event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) {
            Err(LinuxKernelError::EINVAL)?
        }

        let current_event = epoll
            .interest_list
            .get_mut(&fd)
            .ok_or(LinuxKernelError::EINVAL)?;

        if current_event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) {
            Err(LinuxKernelError::EINVAL)?
        }

        *current_event = event;
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
        Ok(self
            .system
            .fd_map
            .borrow_mut()
            .insert(&self.task_id, fd)?
            .into())
    }
}

#[derive(Debug)]
pub struct EpollData {
    interest_list: BTreeMap<FD, epoll_event>,
}

impl EpollData {
    fn new() -> Self {
        EpollData {
            interest_list: Default::default(),
        }
    }
}

#[derive(Debug, Decode, Encode, RequiredMem)]
#[repr(C)]
struct epoll_event {
    events: EPOLL_EVENTS,
    data: u64,
}

bitflags! {
    #[derive(Debug, RequiredMem)]
    #[repr(C)]
    pub struct EPOLL_EVENTS: u32 {
        const EPOLLIN = 0x001;
        const EPOLLPRI = 0x002;
        const EPOLLOUT = 0x004;
        const EPOLLRDNORM = 0x040;
        const EPOLLRDBAND = 0x080;
        const EPOLLWRNORM = 0x100;
        const EPOLLWRBAND = 0x200;
        const EPOLLMSG = 0x400;
        const EPOLLERR = 0x008;
        const EPOLLHUP = 0x010;
        const EPOLLRDHUP = 0x2000;
        const EPOLLEXCLUSIVE = 1 << 28;
        const EPOLLWAKEUP = 1 << 29;
        const EPOLLONESHOT = 1 << 30;
        const EPOLLET = 1 << 31;
    }
}

impl Decode for EPOLL_EVENTS {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        Self::from_bits(Endianness::read_u32(val)).ok_or(MemoryError::ConversionError)
    }
}

impl Encode for EPOLL_EVENTS {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        self.bits().encode::<Endianness>(buffer)
    }
}

#[allow(non_camel_case_types)]
#[derive(TryFromPrimitive)]
#[repr(u64)]
pub enum EPOLL_OPCODES {
    EPOLL_CTL_ADD = 1,
    EPOLL_CTL_DEL = 2,
    EPOLL_CTL_MOD = 3,
}
#[derive(Debug)]
pub struct EpollOps {}

impl EpollOps {
    fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for EpollOps {
    fn supports_poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> bool {
        true
    }
}
