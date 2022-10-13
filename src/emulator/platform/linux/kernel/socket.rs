use std::cmp::min;
use std::mem;

use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::{CPU, Vaddr};
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::{LinuxKernel, LinuxKernelError, SyscallResult};
use crate::emulator::system::memory::{Decode, MemoryError, MemoryWrite, RequiredMem};
use crate::emulator::system::net::{netlink, Sockaddr, SocketDomain, SocketFlags, SocketType};

impl<Endianness: ByteOrder, Arch: Architecture>
LinuxKernel<Endianness, Arch> {
    pub fn socket(
        &mut self,
        _: &mut impl CPU,
        family: u64,
        typ: u64,
        protocol: u64,
    ) -> SyscallResult {
        let family = SocketDomain::from(family);
        let flags = SocketFlags::from(typ);
        let typ = SocketType::try_from(typ)?;
        debug!("Creating socket ({:?} {:?})", family, typ);
        use SocketDomain::*;
        let socket: Box<dyn Handle<Endianness>> = match family {
            AF_NETLINK => netlink::from_protocol(protocol)?,
            _ => todo!()
        };
        Ok(self.handles.add(socket))
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
        dest_addr: Vaddr, // sockaddr*
        dest_addr_len: u64,
    ) -> SyscallResult {
        let sockaddr = self.memory.read_struct_bounded::<Sockaddr>(dest_addr, dest_addr_len as usize)?;
        let buff = self.memory.access_mem(buff, len as usize)?;
        debug!("Buff: {:?}", &*buff);
        Ok(self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADFD)?.sendto(&*buff)? as u64)
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
        let msghdr = self.memory.read_struct::<user_msghdr>(msg)?;
        let addr = self.memory.read_struct::<Sockaddr>(msghdr.msg_name);
        let iovec = self.memory.read_structs::<iovec>(msghdr.msg_iov, msghdr.msg_iovlen as usize)?;
        let to_read = iovec.iter().try_fold(0usize, |sum, vec| sum.checked_add(vec.iov_len as usize)).ok_or(LinuxKernelError::EINVAL)?;
        let data = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADFD)?.recvmsg(to_read)?;
        let mut offset = 0;
        for vec in iovec {
            let to_write = &data[offset..];
            let max_len = min(vec.iov_len as usize, to_write.len());
            offset += self.memory.write(vec.iov_base, &to_write[..max_len]) as usize;
            if max_len < vec.iov_len as usize {
                break
            }
        }
        Ok(offset as u64)
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
        cpu: &mut impl CPU,
        fd: u64,
        umyaddr: Vaddr, // sockaddr*
        addrlen: u64,
    ) -> SyscallResult {
        let handle = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADF)?;
        let mut sockaddr = self.memory.read_struct::<Sockaddr>(umyaddr)?;
        if let Sockaddr::NL { nl_pad, nl_pid, nl_groups } = &mut sockaddr {
            if *nl_pid == 0 {
                *nl_pid = self.pid;
            }
        }
        handle.bind(sockaddr)?;
        Ok(0)
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
        let sockaddr = self.handles.get(&fd).ok_or(LinuxKernelError::EBADF)?.getsockname()?;
        self.memory.write_generic(usockaddr, sockaddr)?;
        Ok(0)
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

#[repr(C)]
struct
user_msghdr {
    msg_name: Vaddr,
    /* ptr to socket address structure */ // void*
    msg_namelen: u64,
    /* size of socket address structure */
    msg_iov: Vaddr,
    /* scatter/gather array */ // iovec*
    msg_iovlen: u64,
    /* # elements in msg_iov */
    msg_control: Vaddr,
    /* ancillary data */ // void*
    msg_controllen: u64,
    /* ancillary data buffer length */
    msg_flags: u32,
    /* flags on received message */
}

impl<Endianness: ByteOrder>
Decode<Endianness> for
user_msghdr {
    fn decode(
        val: &[u8]
    ) -> Result<Self, MemoryError> {
        Ok(Self {
            msg_name: Endianness::read_u64(&val[..8]),
            msg_namelen: Endianness::read_u64(&val[8..16]),
            msg_iov: Endianness::read_u64(&val[16..24]),
            msg_iovlen: Endianness::read_u64(&val[24..32]),
            msg_control: Endianness::read_u64(&val[32..40]),
            msg_controllen: Endianness::read_u64(&val[40..48]),
            msg_flags: Endianness::read_u32(&val[48..52]),
        })
    }
}

impl
RequiredMem for
user_msghdr {}

#[repr(C)]
struct
iovec {
    iov_base: Vaddr,
    /* BSD uses caddr_t (1003.1g requires void *) */ // void*
    iov_len: u64,
    /* Must be size_t (1003.1g) */
}

impl<Endianness: ByteOrder>
Decode<Endianness> for
iovec {
    fn decode(
        val: &[u8]
    ) -> Result<Self, MemoryError> {
        Ok(Self {
            iov_base: Endianness::read_u64(&val[..8]),
            iov_len: Endianness::read_u64(&val[8..16]),
        })
    }
}

impl
RequiredMem for
iovec {}