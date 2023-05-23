use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::{EVAddr, EVA_NULL};
use crate::emulator::abstractions::mmu::{DirectMemoryWrite, MMU, MREMAP_FLAGS};
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::{LinuxKernel, LinuxKernelError, SyscallResult};

// TODO: Bitflags?

const MAP_ANONYMOUS: u64 = 0x20;
const MAP_PRIVATE: u64 = 0x2;
const MAP_FIXED: u64 = 0x10;
const MAP_DENYWRITE: u64 = 0x0800;

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernel<Endianness, Arch> {
    fn mmap_anon(
        &mut self,
        mmu: &mut impl MMU,
        addr: EVAddr,
        length: u64,
        prot: u64,
        flags: u64,
    ) -> Result<EVAddr, LinuxKernelError> {
        let addr = if (flags & MAP_FIXED) != 0 {
            self.memory
                .borrow_mut()
                .unmap(self.task_id, addr, length as usize)?;
            addr
        } else {
            //self.memory.find_free(addr, length)
            addr
        };
        let addr = match self
            .memory
            .borrow_mut()
            .mmap(self.task_id, addr, length as usize, prot.try_into()?)
        {
            Ok(addr) => addr,
            Err(e) => {
                println!("mmap error {:?}", e);
                return Err(LinuxKernelError::EINVAL);
            }
        };
        Ok(addr)
    }

    fn mmap_private(
        &mut self,
        mmu: &mut impl MMU,
        addr: EVAddr,
        length: u64,
        prot: u64,
        flags: u64,
        fd: u64,
        offset: u64,
    ) -> Result<EVAddr, LinuxKernelError> {
        // TODO: This will leak memory if the file handle errors. Needs to be fixed when converting to uctlb solution.
        let addr = self.mmap_anon(mmu, addr, length, prot, flags)?;
        if fd >= 1 << 31 {
            Err(LinuxKernelError::EINVAL)?
        }
        let addr = self.mmap_anon(mmu, addr, length, prot, flags).unwrap();
        if addr == !EVA_NULL {
            Err(LinuxKernelError::EINVAL)?
        }
        let f = self.handles.get(&fd).ok_or(LinuxKernelError::EINVAL)?;
        let data = f.pread(offset, length)?;
        self.mem_accessor(mmu).write_direct(addr, data.as_slice())?;
        Ok(addr)
    }

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
        if (flags & !(MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED | MAP_DENYWRITE)) != 0 {
            Err(LinuxKernelError::EINVAL)?
        }
        if (flags & MAP_ANONYMOUS) != 0 {
            Ok(self.mmap_anon(cpu, addr, length, prot, flags)?.into())
        } else if (flags & MAP_PRIVATE) != 0 {
            Ok(self
                .mmap_private(cpu, addr, length, prot, flags, fd, offset)?
                .into())
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
        self.memory
            .borrow_mut()
            .unmap(self.task_id, addr, len as usize)?;
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
        self.memory
            .borrow()
            .protect(self.task_id, start, len as usize, prot.try_into().unwrap())?;
        Ok(0)
    }
}
