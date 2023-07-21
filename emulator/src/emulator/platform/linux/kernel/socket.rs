use std::cmp::min;

use byteorder::ByteOrder;

use fuzzyemu_derive::{Decode, Encode, RequiredMem};

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::mmu::encoding::{Decode, RequiredMem};
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::{FileDescriptorData, FD};
use crate::emulator::platform::linux::kernel::net::unix::UnixSocketOps;
use crate::emulator::platform::linux::kernel::net::{inet6, netlink, Sockaddr, SocketDomain, SocketFlags, SocketType};
use crate::emulator::platform::linux::kernel::{LinuxKernelError, LinuxKernelTask, SyscallResult};
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::memory::rw_interface::{DirectMemoryWrite, MemoryRead, MemoryWrite};

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
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
        let socket_ops: Box<dyn FileDescriptorOps<FileDescriptorData>> = match family {
            AF_NETLINK => netlink::from_protocol::<Endianness>(protocol)?,
            AF_UNIX => Box::new(UnixSocketOps::new()),
            AF_INET6 => inet6::from_type(typ)?,
            _ => todo!(),
        };
        let fd = FileDescriptor::new(socket_ops);
        Ok(self
            .system
            .fd_map
            .borrow_mut()
            .insert(&self.task_id, fd)?
            .into())
    }

    pub fn connect(
        &mut self,
        _: &mut impl CPU,
        fd: FD,
        uservaddr: EVAddr,
        addrlen: u64,
    ) -> SyscallResult {
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        fd.ops.connect(&mut fd.data)?;
        Ok(0)
    }

    pub fn accept(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        upeer_sockaddr: EVAddr, // sockaddr*
        upeer_addrlen: EVAddr,  // int*
    ) -> SyscallResult {
        todo!()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sendto(
        &mut self,
        mmu: &mut impl MMU,
        fd: FD,
        buf: EVAddr,       // void*
        len: u64,          // size_t
        flags: u64,        // unsigned int
        dest_addr: EVAddr, // sockaddr*
        dest_addr_len: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        let sockaddr: Sockaddr = memory.read_bounded(dest_addr, dest_addr_len as usize)?;
        let buf = memory.read_bytes(buf, len as usize)?;
        debug!("Buff: {:?}", &*buf);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        Ok(fd.ops.sendto(&mut fd.data, &buf)? as u64)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn recvfrom(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        ubuf: EVAddr,     // void*
        size: u64,        // size_t
        flags: u64,       // unsigned int
        addr: EVAddr,     // sockaddr*
        addr_len: EVAddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn sendmsg(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        msg: EVAddr, // msghdr*
        flags: u64,  // unsigned int
    ) -> SyscallResult {
        todo!()
    }

    pub fn recvmsg(
        &mut self,
        mmu: &mut impl MMU,
        fd: FD,
        msg: EVAddr, // msghdr*
        flags: u64,  // unsigned int
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let msghdr: user_msghdr = memory.read(msg)?;
        let addr: Sockaddr = memory.read(msghdr.msg_name)?;
        let iovec: Vec<iovec> = memory.read_n(msghdr.msg_iov, msghdr.msg_iovlen as usize)?;
        let to_read = iovec
            .iter()
            .try_fold(0usize, |sum, vec| sum.checked_add(vec.iov_len as usize))
            .ok_or(LinuxKernelError::EINVAL)?;
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        let data = fd.ops.recvmsg(&mut fd.data, to_read)?;
        let mut offset = 0;
        for vec in iovec {
            let to_write = &data[offset..];
            let max_len = min(vec.iov_len as usize, to_write.len());
            offset += memory.write_direct(vec.iov_base, &to_write[..max_len])? as usize;
            if max_len < vec.iov_len as usize {
                break;
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
        mmu: &mut impl MMU,
        fd: FD,
        umyaddr: EVAddr, // sockaddr*
        addrlen: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let fd = &mut *fd;
        let mut sockaddr = memory.read(umyaddr)?;
        if let Sockaddr::NL { nl_pid, .. } = &mut sockaddr {
            if *nl_pid == 0 {
                *nl_pid = self.task_id.pid;
            }
        }
        fd.ops.bind(&mut fd.data, sockaddr)?;
        Ok(0)
    }

    pub fn listen(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        backlog: u64,
    ) -> SyscallResult {
        error!("TODO: Listen only has a dummy implementation for now!");
        Ok(0)
    }

    pub fn getsockname(
        &mut self,
        mmu: &mut impl MMU,
        fd: FD,
        usockaddr: EVAddr,     // sockaddr*
        usockaddr_len: EVAddr, // int*
    ) -> SyscallResult {
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        let sockaddr = fd.ops.getsockname(&mut fd.data)?;
        self.system
            .memory
            .borrow_mut()
            .accessor(mmu, self.task_id)
            .write(usockaddr, sockaddr)?;
        Ok(0)
    }

    pub fn getpeername(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        usockaddr: EVAddr,     // sockaddr*
        usockaddr_len: EVAddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn socketpair(
        &mut self,
        _: &mut impl CPU,
        family: u64,
        typ: u64,
        protocol: u64,
        usockvec: EVAddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn setsockopt(
        &mut self,
        mmu: &mut impl MMU,
        fd: FD,
        level: u64,
        optname: u64,
        optval: EVAddr, // char*
        optlen: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        let opt_data = memory.read_bytes(optval, optlen as usize)?;
        // TODO: Handle sockopts correctly
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        fd.ops.setsockopt(&mut fd.data)?;
        Ok(0)
    }

    pub fn getsockopt(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        level: u64,
        optname: u64,
        optval: EVAddr, // char*
        optlen: EVAddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn accept4(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        upeer_sockaddr: EVAddr, // sockaddr*
        upeer_addrlen: EVAddr,  // int*
        flags: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn recvmmsg(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        mmsg: EVAddr,    // mmsghdr*
        vlen: u64,       // unsigned int
        flags: u64,      // unsigned int
        timeout: EVAddr, // timespec*
    ) -> SyscallResult {
        todo!()
    }

    pub fn sendmmsg(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
        mmsg: EVAddr, // mmsghdr*
        vlen: u64,    // unsigned int
        flags: u64,   // unsigned int
    ) -> SyscallResult {
        todo!()
    }
}

#[derive(Encode, Decode, RequiredMem)]
#[repr(C)]
struct user_msghdr {
    msg_name: EVAddr,
    /* ptr to socket address structure */ // void*
    msg_namelen: u64,
    /* size of socket address structure */
    msg_iov: EVAddr,
    /* scatter/gather array */ // iovec*
    msg_iovlen: u64,
    /* # elements in msg_iov */
    msg_control: EVAddr,
    /* ancillary data */ // void*
    msg_controllen: u64,
    /* ancillary data buffer length */
    msg_flags: u32,
    /* flags on received message */
}

#[derive(Decode, Encode, RequiredMem)]
#[repr(C)]
struct iovec {
    iov_base: EVAddr,
    /* BSD uses caddr_t (1003.1g requires void *) */ // void*
    iov_len: u64,
    /* Must be size_t (1003.1g) */
}
