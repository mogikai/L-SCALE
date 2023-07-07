use std::ops::Deref;
use bitflags::bitflags;
use byteorder::ByteOrder;
use iced_x86::OpCodeOperandKind::mem;

use crate::emulator::platform::linux::kernel::*;
use crate::emulator::system::memory::rw_interface::{DirectMemoryWrite, MemoryWrite};
use crate::emulator::macros::*;

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    pub fn write(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        buf: EVAddr,
        size: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        self.handles
            .get_mut(&fd)
            .map_or(Err(LinuxKernelError::EINVAL), |handle| {
                Ok(handle
                    .write(
                        memory
                            .accessor(mmu, self.task_id)
                            .read_bytes(buf, size as usize)?
                            .as_slice(),
                    )
                    .unwrap_or(!0)
                    .try_into()
                    .unwrap())
            })
    }

    pub fn writev(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        iovec_ptr: EVAddr,
        iovcnt: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        self.handles
            .get_mut(&fd)
            .map_or(Err(LinuxKernelError::EINVAL), |handle| {
                let mut ret = 0;
                let iovec = memory
                    .read_bytes(iovec_ptr, (iovcnt * 16) as usize)
                    .unwrap();
                for i in 0..iovcnt as usize {
                    let ptr = EVAddr::from(Endianness::read_u64(&iovec[(i * 16)..((i * 16) + 8)]));
                    let size = Endianness::read_u64(&iovec[i * 16 + 8..i * 16 + 16]) as usize;
                    let data = memory.read_bytes(ptr, size)?;
                    let written = handle.write(data.as_ref())?;
                    ret += written;
                    // Early return if not all bytes were written. We need to stop writing in this case
                    if written != data.len() {
                        return Ok(ret.try_into().unwrap());
                    };
                }
                Ok(ret.try_into().unwrap())
            })
    }

    pub fn pread64(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        buf: EVAddr,
        count: u64,
        pos: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let file = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADF)?;
        let d = file.pread(pos, count).unwrap();
        Ok(memory.write_direct(buf, d.as_slice())? as u64)
    }

    pub fn read(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        buf: EVAddr,
        count: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        if let Some(f) = self.handles.get_mut(&fd) {
            Ok(memory.write_direct(buf, f.read(count).unwrap().as_slice())? as u64)
        } else {
            Err(LinuxKernelError::EINVAL)
        }
    }

    pub fn fstat(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        statbuf: EVAddr,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let file = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADF)?;
        memory.write(statbuf, file.stat()?)?;
        Ok(0)
    }

    pub fn stat(
        &mut self,
        mmu: &mut impl MMU,
        filename: EVAddr,
        statbuf: EVAddr,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let fname = read_osstring!(memory, filename)?;
        trace!("stat file {:?}", fname);
        let fs = self.system.filesystem.borrow();
        let file = fs.resolve_last(Path::new(&fname))?;
        memory.write(statbuf, fs.stat(file)?)?;
        Ok(0)
    }

    pub fn openat(
        &mut self,
        mmu: &mut impl MMU,
        _dfd: u64,
        filename: EVAddr,
        _flags: u64,
        _mode: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        let fname = read_osstring!(memory, filename)?;
        trace!("openat file: {:?}", fname);
        let file = self
            .system
            .filesystem
            .borrow()
            .resolve_last(Path::new(&fname))?;
        Ok(self
            .handles
            .add(Box::new(FileHandle::new(self.system.clone(), file))))
    }

    pub fn statfs(
        &mut self,
        _: &mut impl CPU,
        path: EVAddr,
        buf: EVAddr,
    ) -> SyscallResult {
        todo!()
    }

    pub fn newfstatat(
        &mut self,
        mmu: &mut impl MMU,
        dfd: u64,
        filename: EVAddr,
        statbuf: EVAddr,
        flag: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let flags = at_flags::from_bits(flag).ok_or(LinuxKernelError::EINVAL)?;
        if flags.contains(at_flags::AT_EMPTY_PATH) {
            let file = self.handles.get(&dfd).ok_or(LinuxKernelError::EBADFD)?;
            memory.write(statbuf, file.stat()?)?;
            Ok(0)
        } else {
            let filename = read_osstring!(memory, filename)?;
            debug!("newfstatat: {filename:?}");
            let fs = self.system.filesystem.borrow_mut();
            let entry = fs.resolve_last(Path::new(&filename))?;
            memory.write(statbuf, fs.stat(entry)?)?;
            Ok(0)
        }
    }

    pub fn close(
        &mut self,
        _: &mut impl CPU,
        fd: u64,
    ) -> SyscallResult {
        if self.handles.remove(&fd).is_none() {
            Err(LinuxKernelError::EBADF)?
        }
        Ok(0)
    }

    pub fn add_replacement_handle(
        &mut self,
        fd: u64,
        handle: Box<dyn Handle>,
    ) {
        self.handles.add_replacement_handle(fd, handle);
    }
}

const AT_FDCWD: u64 = -100i32 as u64; /* Special value used to indicate the *at functions should use the current working directory. */

bitflags! {
    #[allow(non_camel_case_types)]
    pub struct
    at_flags: u64 {
        const AT_SYMLINK_NOFOLLOW =	0x100;	/* Do not follow symbolic links.  */
        const AT_REMOVEDIR =		0x200;	/* Remove directory instead of unlinking file.  */
        const AT_SYMLINK_FOLLOW =	0x400;	/* Follow symbolic links.  */
        const AT_NO_AUTOMOUNT =	0x800;	/* Suppress terminal automount traversal.  */
        const AT_EMPTY_PATH =		0x1000;	/* Allow empty relative pathname.  */
        const AT_STATX_SYNC_TYPE =	0x6000;
        const AT_STATX_SYNC_AS_STAT =	0x0000;
        const AT_STATX_FORCE_SYNC =	0x2000;
        const AT_STATX_DONT_SYNC =	0x4000;
        const AT_RECURSIVE =		0x8000;	/* Apply to the entire subtree.  */
        const AT_EACCESS =		0x200;	/* Test access permitted for effective IDs, not real IDs.  */
    }
}
