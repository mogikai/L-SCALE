use std::collections::BTreeMap;

use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::{LinuxKernelTask, SyscallResult};
use crate::emulator::platform::linux::kernel::FileDescriptor;

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
        _: &mut impl CPU,
        epfd: u64,
        op: u64,
        fd: u64,
        event: EVAddr, // epoll_event*
    ) -> SyscallResult {
        // TODO: The poll and select syscalls require files to have a poll/select function that can be called by
        //       the epoll/select handler to query whether or not data is available for read or write ops
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
        fd.data.epoll = Some(Epoll::new());
        Ok(self.fd_map.insert(&mut self.system.fd_map.borrow_mut(),fd).into())
    }
}

#[derive(Debug)]
pub struct Epoll {
    interest_list: BTreeMap<u64, u64>,
}

impl Epoll {
    fn new() -> Self { Epoll { interest_list: Default::default() } }
}

#[derive(Debug)]
pub struct EpollOps {}

impl EpollOps {
    fn new() -> Self { Self {} }
}

impl FileDescriptorOps for EpollOps {}
