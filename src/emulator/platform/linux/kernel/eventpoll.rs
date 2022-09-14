use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::{CPU, Vaddr};
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
        events: Vaddr, // epoll_event*
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
        event: Vaddr, // epoll_event*
    ) -> SyscallResult {
        todo!()
    }

    pub fn epoll_pwait(
        &mut self,
        _: &mut impl CPU,
        epfd: u64,
        events: Vaddr, // epoll_event*
        maxevents: u64,
        timeout: u64,
        sigmask: Vaddr, // sigset_t*
        sigsetsize: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn epoll_create1(
        &mut self,
        _: &mut impl CPU,
        flags: u64,
    ) -> SyscallResult {
        todo!()
    }
}