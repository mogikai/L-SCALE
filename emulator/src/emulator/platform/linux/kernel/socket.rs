use std::cmp::min;

use byteorder::ByteOrder;
use tracing::{
    debug,
    error,
};

use mogi_derive::{
    Decode,
    Encode,
    RequiredMem,
};
use mogi_utilities::addresses::{
    EVAddr,
    EVA_NULL,
};
use mogi_utilities::encoding::{
    Encode,
    RequiredMem,
};

use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::network::SocketAddress;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::FD;
use crate::emulator::platform::linux::kernel::net::sockopt::Sockopt;
use crate::emulator::platform::linux::kernel::net::unix::UnixSocketOps;
use crate::emulator::platform::linux::kernel::net::{
    inet6,
    netlink,
    ShutMethod,
    Sockaddr,
    SocketDomain,
    SocketFlags,
    SocketType,
};
use crate::emulator::platform::linux::kernel::{
    sys_component,
    sys_component_mut,
    LinuxKernelError,
    LinuxKernelTask,
    SyscallResult,
};
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::memory::rw_interface::{
    DirectMemoryWrite,
    MemoryRead,
    MemoryWrite,
};

impl<Endianness, Arch, CpuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder + 'static,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem + 'static,
{
    pub fn socket(
        &mut self,
        family: u64,
        typ: u64,
        protocol: u64,
    ) -> SyscallResult {
        let family = SocketDomain::from(family);
        let flags = SocketFlags::from(typ);
        let typ = SocketType::try_from(typ)?;
        debug!("Creating socket ({:?} {:?})", family, typ);
        use SocketDomain::*;
        let fd = match family {
            AF_NETLINK => netlink::from_protocol::<Endianness>(protocol)?,
            AF_UNIX => FileDescriptor::new(Box::new(UnixSocketOps::new())),
            AF_INET | AF_INET6 => inet6::from_type(typ, &self.system)?,
            _ => todo!(),
        };
        Ok(sys_component_mut!(self, fd_map)
            .insert(&self.task_id, fd)?
            .into())
    }

    pub fn connect(
        &mut self,
        fd: FD,
        uservaddr: EVAddr,
        addrlen: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut cpu, self.task_id);

        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;

        let sockaddr: Sockaddr = memory.read_bounded(uservaddr, addrlen as usize)?;

        fd.connect(sockaddr.into())?;
        Ok(0)
    }

    pub fn accept(
        &mut self,
        fd: FD,
        upeer_sockaddr: EVAddr, // sockaddr*
        upeer_addrlen: EVAddr,  // int*
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut cpu, self.task_id);

        let mut fd_map = sys_component_mut!(self, fd_map);

        let new_fd = {
            let mut fd = fd_map
                .get_mut(&self.task_id, &fd)
                .ok_or(LinuxKernelError::EBADF)?;
            let fd = &mut *fd;

            let (new_fd, peer) = fd.accept()?;

            let addrlen: i32 = memory.read(upeer_addrlen)?;
            if addrlen > 0 && upeer_sockaddr != EVA_NULL {
                let (_, total) = memory.write_bounded(upeer_sockaddr, Sockaddr::from(peer), addrlen as usize)?;
                memory.write(upeer_addrlen, total as i32);
            }
            new_fd
        };

        Ok(fd_map.insert(&self.task_id, new_fd)?.into())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sendto(
        &mut self,
        fd: FD,
        buf: EVAddr,       // void*
        len: u64,          // size_t
        flags: u64,        // unsigned int
        dest_addr: EVAddr, // sockaddr*
        dest_addr_len: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let memory = memory.accessor(&mut cpu, self.task_id);
        let sockaddr: Option<Sockaddr> = if dest_addr_len != 0 && dest_addr != EVA_NULL {
            Some(memory.read_bounded(dest_addr, dest_addr_len as usize)?)
        } else {
            None
        };
        let buf = memory.read_bytes(buf, len as usize)?;
        debug!("Buff: {:?}", &*buf);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        Ok(fd.sendto(&buf, &sockaddr.map(SocketAddress::from))? as u64)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn recvfrom(
        &mut self,
        fd: FD,
        ubuf: EVAddr,     // void*
        size: u64,        // size_t
        flags: u64,       // unsigned int
        addr: EVAddr,     // sockaddr*
        addr_len: EVAddr, // int*
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut cpu, self.task_id);

        let mut fd_map = sys_component_mut!(self, fd_map);
        let mut fd = fd_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let fd = &mut *fd;

        let (data, sender) = fd.recvfrom(size as usize)?;

        let (written, _) = memory.write_bounded(ubuf, data.as_slice(), size as usize)?;

        if addr_len != EVA_NULL && addr != EVA_NULL {
            let sockaddr_len: i32 = memory.read(addr_len)?;
            let (_, total) = memory.write_bounded(addr, Sockaddr::from(sender), sockaddr_len as usize)?;
            memory.write(addr_len, total as i32)?;
        }
        Ok(written as u64)
    }

    pub fn sendmsg(
        &mut self,
        fd: FD,
        msg: EVAddr, // msghdr*
        flags: u64,  // unsigned int
    ) -> SyscallResult {
        todo!()
    }

    pub fn recvmsg(
        &mut self,
        fd: FD,
        msg: EVAddr, // msghdr*
        flags: u64,  // unsigned int
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut cpu, self.task_id);
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
        let data = fd.recvmsg(to_read)?;
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
        fd: FD,
        how: ShutMethod,
    ) -> SyscallResult {
        let mut fd_map = sys_component_mut!(self, fd_map);
        let fd = fd_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        fd.shutdown(how.into())?;
        Ok(0)
    }

    pub fn bind(
        &mut self,
        fd: FD,
        umyaddr: EVAddr, // sockaddr*
        addrlen: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let memory = memory.accessor(&mut cpu, self.task_id);
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
        fd.bind(sockaddr.into())?;
        Ok(0)
    }

    pub fn listen(
        &mut self,
        fd: FD,
        backlog: usize,
    ) -> SyscallResult {
        let mut fd_map = sys_component_mut!(self, fd_map);
        let mut fd = fd_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let fd = &mut *fd;
        fd.listen(backlog)?;
        Ok(0)
    }

    pub fn getsockname(
        &mut self,
        fd: FD,
        usockaddr: EVAddr,     // sockaddr*
        usockaddr_len: EVAddr, // int*
    ) -> SyscallResult {
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        let sockaddr = Sockaddr::from(*fd.getsockname()?);
        let mut cpu = sys_component_mut!(self, cpu);
        sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write(usockaddr, sockaddr)?;
        Ok(0)
    }

    pub fn getpeername(
        &mut self,
        fd: FD,
        usockaddr: EVAddr,     // sockaddr*
        usockaddr_len: EVAddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn socketpair(
        &mut self,
        family: u64,
        typ: u64,
        protocol: u64,
        usockvec: EVAddr, // int*
    ) -> SyscallResult {
        todo!()
    }

    pub fn setsockopt(
        &mut self,
        fd: FD,
        level: u64,
        optname: u64,
        optval: EVAddr, // char*
        optlen: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let memory = memory.accessor(&mut cpu, self.task_id);
        // TODO: Read the optval variant depending on level and optname
        // let optval: Optval = memory.read_bounded(optval, optlen);
        // FIXME: Handle sockopts correctly
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        error!("setsockopt currently not implemented correctly");
        fd.setsockopt(Sockopt::try_from((level, optname))?)?;
        Ok(0)
    }

    pub fn getsockopt(
        &mut self,
        fd: FD,
        level: u64,
        optname: u64,
        optval_addr: EVAddr, // char*
        optlen_ptr: EVAddr,  // int*
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut cpu, self.task_id);
        let optlen: usize = memory.read(optlen_ptr)?;
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADFD)?;
        let fd = &mut *fd;
        let optval = fd.getsockopt(Sockopt::try_from((level, optname))?)?;
        let (actual, _) = memory.write_bounded(optval_addr, optval, optlen)?;
        memory.write(optlen_ptr, actual)?;
        Ok(0)
    }

    pub fn accept4(
        &mut self,
        fd: FD,
        upeer_sockaddr: EVAddr, // sockaddr*
        upeer_addrlen: EVAddr,  // int*
        flags: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn recvmmsg(
        &mut self,
        fd: FD,
        mmsg: EVAddr,    // mmsghdr*
        vlen: u64,       // unsigned int
        flags: u64,      // unsigned int
        timeout: EVAddr, // timespec*
    ) -> SyscallResult {
        todo!()
    }

    pub fn sendmmsg(
        &mut self,
        fd: FD,
        mmsg: EVAddr, // mmsghdr*
        vlen: u64,    // unsigned int
        flags: u64,   // unsigned int
    ) -> SyscallResult {
        todo!()
    }
}

#[derive(Debug)]
pub enum Optval {
    SocketOptval(i32),
}

impl RequiredMem for Optval {
    fn required_mem() -> usize {
        unimplemented!()
    }
}

impl Encode for Optval {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        match self {
            Optval::SocketOptval(val) => val.encode::<Endianness>(buffer),
        }
    }

    fn encoded_size(&self) -> usize {
        match self {
            Optval::SocketOptval(val) => val.encoded_size(),
        }
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
