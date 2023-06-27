use bitflags::bitflags;
use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::{EVAddr, EVA_NULL};
use crate::emulator::abstractions::mmu::{MMU, MREMAP_FLAGS};
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::memory::flags::MMAP_FLAGS;
use crate::emulator::platform::linux::kernel::{LinuxKernelError, LinuxKernelTask, SyscallResult};
use crate::emulator::system::memory::rw_interface::{DirectMemoryWrite, MemoryWrite};

pub mod flags;

////////////////////////////////
//// Public Implementations ////
////////////////////////////////

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    pub fn mmap(
        &mut self,
        cpu: &mut impl MMU,
        addr: EVAddr,
        length: u64,
        prot: u64,
        flags: u64,
        fd: u64,
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

        if flags.contains(MMAP_FLAGS::MAP_ANONYMOUS) {
            Ok(self.mmap_anon(cpu, addr, length, prot, flags)?.into())
        } else if flags.contains(MMAP_FLAGS::MAP_PRIVATE) {
            Ok(self
                .mmap_private(cpu, addr, length, prot, flags, fd, offset)?
                .into())
        } else if flags.contains(MMAP_FLAGS::MAP_SHARED_VALIDATE) {
            todo!("Implement MAP_SHARED_VALIDATE")
        } else {
            Err(LinuxKernelError::EINVAL)
        }
    }

    pub fn munmap(
        &mut self,
        mmu: &mut impl MMU,
        addr: EVAddr,
        len: u64,
    ) -> SyscallResult {
        self.system
            .memory
            .borrow_mut()
            .unmap(mmu, self.task_id, addr, len as usize)?;
        Ok(0)
    }

    pub fn mremap(
        &mut self,
        cpu: &mut impl MMU,
        old_addr: EVAddr,
        old_len: u64,
        new_len: u64,
        flags: u64,
        new_addr: EVAddr,
    ) -> SyscallResult {
        Ok(self
            .system
            .memory
            .borrow()
            .remap(
                self.task_id,
                old_addr,
                old_len,
                new_addr,
                new_len,
                MREMAP_FLAGS::from_bits(flags).ok_or(LinuxKernelError::EINVAL)?,
            )?
            .into())
    }

    pub fn mprotect(
        &mut self,
        _: &mut impl MMU,
        start: EVAddr,
        len: u64,
        prot: u64,
    ) -> SyscallResult {
        self.system
            .memory
            .borrow()
            .protect(self.task_id, start, len as usize, prot.try_into().unwrap())?;
        Ok(0)
    }
}

/////////////////////////////////
//// Private Implementations ////
/////////////////////////////////

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    fn mmap_anon(
        &mut self,
        mmu: &mut impl MMU,
        addr: EVAddr,
        length: u64,
        prot: u64,
        flags: MMAP_FLAGS,
    ) -> Result<EVAddr, LinuxKernelError> {
        if flags.contains(MMAP_FLAGS::MAP_FIXED) {
            self.system
                .memory
                .borrow_mut()
                .unmap(mmu, self.task_id, addr, length as usize)?;

            self.system
                .memory
                .borrow_mut()
                .map_fixed(mmu, self.task_id, addr, length as usize, prot.try_into()?)
                .map_err(|e| e.into())
        } else {
            self.system
                .memory
                .borrow_mut()
                .mmap(mmu, self.task_id, addr, length as usize, prot.try_into()?)
                .map_err(|e| e.into())
        }
    }

    fn mmap_private(
        &mut self,
        mmu: &mut impl MMU,
        addr: EVAddr,
        length: u64,
        prot: u64,
        flags: MMAP_FLAGS,
        fd: u64,
        offset: u64,
    ) -> Result<EVAddr, LinuxKernelError> {
        // TODO: This will leak memory if the file handle errors. Needs to be fixed when converting to uctlb solution.
        let addr = self.mmap_anon(mmu, addr, length, prot, flags)?;
        if fd >= 1 << 31 {
            Err(LinuxKernelError::EINVAL)?
        }
        let addr = self.mmap_anon(mmu, addr, length, prot, flags)?;
        if addr == !EVA_NULL {
            Err(LinuxKernelError::EINVAL)?
        }
        let f = self.handles.get(&fd).ok_or(LinuxKernelError::EINVAL)?;
        let data = f.pread(offset, length)?;

        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        memory.write_direct(addr, data.as_slice())?;
        Ok(addr)
    }
}
