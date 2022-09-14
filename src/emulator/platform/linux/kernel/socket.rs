use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::{CPU, Vaddr};
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::{LinuxKernel, SyscallResult};

impl<Endianness: ByteOrder, Arch: Architecture> LinuxKernel<Endianness, Arch> {
    pub fn socket(
        &mut self,
        _: &mut impl CPU,
        family: u64,
        typ: u64,
        protocol: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn connect(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        uservaddr: Vaddr,
        addrlen: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn accept(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        upeer_sockaddr: Vaddr, // sockaddr*
        upeer_addrlen: Vaddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn sendto(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        buff: Vaddr, // void*
        len: u64, // size_t
        flags: u64, // unsigned int
        addr: Vaddr, // sockaddr*
        addr_len: u64, // int
    ) -> SyscallResult {
        todo!()
    }

    pub fn recvfrom(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        ubuf: Vaddr, // void*
        size: u64, // size_t
        flags: u64, // unsigned int
        addr: Vaddr, // sockaddr*
        addr_len: Vaddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn sendmsg(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        msg: Vaddr, // msghdr*
        flags: u64, // unsigned int
    ) -> SyscallResult {
        todo!()
    }

    pub fn recvmsg(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        msg: Vaddr, // msghdr*
        flags: u64, // unsigned int
    ) -> SyscallResult {
        todo!()
    }

    pub fn shutdown(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        how: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn bind(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        umyaddr: Vaddr, // sockaddr*
        addrlen: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn listen(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        backlog: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn getsockname(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        usockaddr: Vaddr, // sockaddr*
        usockaddr_len: Vaddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn getpeername(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        usockaddr: Vaddr, // sockaddr*
        usockaddr_len: Vaddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn socketpair(
        &mut self,
        _: &mut impl CPU,
        family: u64,
        typ: u64,
        protocol: u64,
        usockvec: Vaddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn setsockopt(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        level: u64,
        optname: u64,
        optval: Vaddr, // char*
        optlen: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn getsockopt(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        level: u64,
        optname: u64,
        optval: Vaddr, // char*
        optlen: Vaddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn accept4(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        upeer_sockaddr: Vaddr, // sockaddr*
        upeer_addrlen: Vaddr, // int*
        flags: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn recvmmsg(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        mmsg: Vaddr, // mmsghdr*
        vlen: u64, // unsigned int
        flags: u64, // unsigned int
        timeout: Vaddr, // timespec*
    ) -> SyscallResult {
        todo!()
    }

    pub fn sendmmsg(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        mmsg: Vaddr, // mmsghdr*
        vlen: u64, // unsigned int
        flags: u64, // unsigned int
    ) -> SyscallResult {
        todo!()
    }
}