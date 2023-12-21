use bitflags::Flags;
use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::{
    EVAddr,
    EVA_NULL,
};
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::mmu::{
    MemoryError,
    Permission,
    MMU,
    MREMAP_FLAGS,
};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::FD;
use crate::emulator::platform::linux::kernel::memory::flags::MMAP_FLAGS;
use crate::emulator::platform::linux::kernel::{
    sys_component,
    sys_component_mut,
    LinuxKernelError,
    LinuxKernelTask,
    SyscallResult,
};
use crate::emulator::system::memory::helpers::{
    align4k_up,
    is4kaligned,
};
use crate::emulator::system::memory::rw_interface::{
    DirectMemoryWrite,
    MemoryWrite,
};
use crate::emulator::system::memory::MappingType;

pub mod flags;

////////////////////////////////
//// Public Implementations ////
////////////////////////////////

impl<Endianness, Arch, CpuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder + 'static,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    #[allow(clippy::too_many_arguments)]
    pub fn mmap(
        &mut self,
        addr: EVAddr,
        length: u64,
        prot: Permission,
        flags: u64,
        fd: FD,
        offset: u64,
    ) -> SyscallResult {
        let flags = MMAP_FLAGS::from_bits_truncate(flags);

        if flags.contains(MMAP_FLAGS::MAP_LOCKED)
            || flags.contains(MMAP_FLAGS::MAP_NORESERVE)
            || flags.contains(MMAP_FLAGS::MAP_POPULATE)
            || flags.contains(MMAP_FLAGS::MAP_STACK)
            || flags.contains(MMAP_FLAGS::MAP_GROWSDOWN)
            || flags.contains(MMAP_FLAGS::MAP_SYNC)
            || flags.contains(MMAP_FLAGS::MAP_HUGETLB)
        {
            todo!()
        }

        if flags.contains(MMAP_FLAGS::MAP_SHARED) {
            todo!("Implement MAP_SHARED")
        } else if flags.contains(MMAP_FLAGS::MAP_PRIVATE) {
            Ok(self
                .mmap_private(addr, length, prot, flags, fd, offset)?
                .into())
        } else if flags.contains(MMAP_FLAGS::MAP_SHARED_VALIDATE) {
            todo!("Implement MAP_SHARED_VALIDATE")
        } else {
            Err(LinuxKernelError::EINVAL.into())
        }
    }

    pub fn munmap(
        &mut self,
        addr: EVAddr,
        len: u64,
    ) -> SyscallResult {
        sys_component_mut!(self, memory).unmap(self.task_id, addr, len as usize)?;
        sys_component_mut!(self, cpu).flush_tlb()?;
        Ok(0)
    }

    pub fn mremap(
        &mut self,
        old_addr: EVAddr,
        old_len: u64,
        new_len: u64,
        flags: u64,
        new_addr: EVAddr,
    ) -> SyscallResult {
        if !is4kaligned(old_addr) {
            Err(MemoryError::InvalidAlignment)?
        }
        if old_len == 0 {
            todo!(
                "The old_len = 0  trick has buggy and inconsistent \
                   behavior on current kernels since 2007 https://bugzilla.kernel.org/show_bug.cgi?id=8691 \
                   and we will not implement it until this is resolved"
            )
        }

        let mut cpu = sys_component_mut!(self, cpu);
        Ok(sys_component_mut!(self, memory)
            .remap(
                &mut *cpu,
                self.task_id,
                old_addr,
                old_len as usize,
                new_addr,
                new_len as usize,
                MREMAP_FLAGS::from_bits(flags).ok_or(LinuxKernelError::EINVAL)?,
            )?
            .into())
    }

    pub fn mprotect(
        &mut self,
        start: EVAddr,
        len: u64,
        prot: u64,
    ) -> SyscallResult {
        if !is4kaligned(start) {
            Err(MemoryError::InvalidAlignment)?
        }
        sys_component_mut!(self, memory).protect(self.task_id, start, len as usize, prot.try_into().unwrap())?;
        // Need to flush existing cache since permissions of cached entries might have changed.
        sys_component_mut!(self, cpu).flush_tlb()?;
        Ok(0)
    }
}

/////////////////////////////////
//// Private Implementations ////
/////////////////////////////////

impl<Endianness, Arch, CpuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder + 'static,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    #[allow(clippy::too_many_arguments)]
    fn mmap_private(
        &mut self,
        addr: EVAddr,
        length: u64,
        prot: Permission,
        flags: MMAP_FLAGS,
        fd: FD,
        offset: u64,
    ) -> Result<EVAddr, LinuxKernelError> {
        if flags.contains(MMAP_FLAGS::MAP_ANONYMOUS) {
            self.mmap_anon(addr, length, prot, flags, MappingType::PrivateAnonymous)
        } else {
            self.mmap_file_private(addr, length, prot, flags, fd, offset)
        }
    }

    fn mmap_anon(
        &mut self,
        address: EVAddr,
        size: u64,
        prot: Permission,
        flags: MMAP_FLAGS,
        mapping_type: MappingType,
    ) -> Result<EVAddr, LinuxKernelError> {
        let mut cpu = sys_component_mut!(self, cpu);
        let size = align4k_up(size as usize);
        let addr = if flags.contains(MMAP_FLAGS::MAP_FIXED) {
            // map fixed overwrites any mappings that already exist in the region so we unmap here.
            sys_component_mut!(self, memory).unmap(self.task_id, address, size)?;
            // Need to flush tlb to let cpu emulator now to re-fetch the addresses
            cpu.flush_tlb()?;

            sys_component_mut!(self, memory)
                .map_fixed(self.task_id, address, size, prot, mapping_type)
                .map_err(LinuxKernelError::from)?
        } else {
            sys_component_mut!(self, memory)
                .mmap(self.task_id, address, size, prot, mapping_type)
                .map_err(LinuxKernelError::from)?
        };

        // Map anonymous requires memory being zero initialized.
        sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write_bytes_unchecked(addr, &vec![0u8; size]);
        Ok(addr)
    }

    #[allow(clippy::too_many_arguments)]
    fn mmap_file_private(
        &mut self,
        addr: EVAddr,
        length: u64,
        prot: Permission,
        flags: MMAP_FLAGS,
        fd: FD,
        offset: u64,
    ) -> Result<EVAddr, LinuxKernelError> {
        if fd <= FD::from(-1) {
            Err(LinuxKernelError::EINVAL)?
        }
        let internal_fd = sys_component_mut!(self, fd_map).dup_internal(&self.task_id, &fd)?;
        let addr = self.mmap_anon(addr, length, prot, flags, MappingType::PrivateFile(internal_fd))?;
        if addr == !EVA_NULL {
            Err(LinuxKernelError::EINVAL)?
        }

        self.write_file_contents(length, fd, offset, addr)
            .map_err(|e| {
                // Unmap if we encountered an error during writing
                sys_component_mut!(self, memory)
                    .unmap(self.task_id, addr, length as usize)
                    .map_err(LinuxKernelError::from)
                    .err()
                    .unwrap_or(e)
            })?;
        Ok(addr)
    }

    fn write_file_contents(
        &mut self,
        length: u64,
        fd: FD,
        offset: u64,
        addr: EVAddr,
    ) -> Result<(), LinuxKernelError> {
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EINVAL)?;
        let fd = &mut *fd;
        let data = fd.pread(offset, length)?;

        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write_bytes_unchecked(addr, data.as_slice())?;
        Ok(())
    }
}
