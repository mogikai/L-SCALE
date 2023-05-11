use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::{LinuxKernel, SyscallResult};

impl<Endianness: ByteOrder, Arch: Architecture> LinuxKernel<Endianness, Arch> {
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
        error!("TODO: Implement epoll_ctl. Need to add fds to interest list");
        Ok(0)
    }

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
        todo!()
    }

    pub fn epoll_create1(
        &mut self,
        _: &mut impl CPU,
        flags: u64,
    ) -> SyscallResult {
        Ok(self.handles.add(Box::new(Epoll::new())))
    }
}

struct Epoll {}

impl Epoll {
    fn new() -> Self { Epoll {} }
}

impl<Endianness: ByteOrder> Handle<Endianness> for Epoll {}
