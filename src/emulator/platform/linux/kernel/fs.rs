use bitflags::bitflags;
use byteorder::ByteOrder;

use crate::emulator::platform::linux::kernel::*;
use crate::read_osstring;

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernel<Endianness, Arch> {
    pub fn write(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        buf: EVAddr,
        size: u64,
    ) -> SyscallResult {
        self.handles.get_mut(&fd).map_or(Err(LinuxKernelError::EINVAL), |handle| {
            Ok(handle.write(&mmu.mem_read_bytes(buf, size as usize)?).unwrap_or(!0).try_into().unwrap())
        })
    }

    pub fn writev(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        iovec_ptr: EVAddr,
        iovcnt: u64,
    ) -> SyscallResult {
        self.handles.get_mut(&fd).map_or(Err(LinuxKernelError::EINVAL), |handle| {
            let mut ret = 0;
            let iovec = mmu.mem_read_bytes(iovec_ptr, (iovcnt * 16) as usize).unwrap();
            for i in 0..iovcnt as usize {
                let ptr = EVAddr::from(Endianness::read_u64(&iovec[(i * 16)..((i * 16) + 8)]));
                let size = Endianness::read_u64(&iovec[i * 16 + 8..i * 16 + 16]) as usize;
                let data = mmu.mem_read_bytes(ptr, size)?;
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
        let file = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADF)?;
        let d = file.pread(pos, count).unwrap();
        Ok(mmu.mem_write_bytes(buf, d.as_slice())? as u64)
    }

    pub fn read(
        &mut self,
        mmu: &mut impl MMU,
        fd: u64,
        buf: EVAddr,
        count: u64,
    ) -> SyscallResult {
        if let Some(f) = self.handles.get_mut(&fd) {
            Ok(mmu.mem_write_bytes(buf, f.read(count).unwrap().as_slice())? as u64)
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
        let file = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADF)?;
        mmu.mem_write_bytes(statbuf, file.stat()?.encode().as_slice())?;
        Ok(0)
    }

    pub fn stat(
        &mut self,
        mmu: &mut impl MMU,
        filename: EVAddr,
        statbuf: EVAddr,
    ) -> SyscallResult {
        let fname = read_osstring!(mmu, filename)?;
        trace!("stat file {:?}", fname);
        let fs = self.filesystem.borrow();
        let file = fs.resolve_last(Path::new(&fname))?;
        mmu.mem_write_bytes(statbuf, fs.stat(file)?.encode().as_slice())?;
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
        let fname = read_osstring!(mmu, filename)?;
        trace!("openat file: {:?}", fname);
        let p = Path::new(&fname);
        let file = self.filesystem.borrow().resolve_last(p)?;
        Ok(self.handles.add(Box::new(FileHandle::new(self.filesystem.clone(), file))))
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
        let flags = at_flags::from_bits(flag).ok_or(LinuxKernelError::EINVAL)?;
        if flags.contains(at_flags::AT_EMPTY_PATH) {
            let file = self.handles.get(&dfd).ok_or(LinuxKernelError::EBADFD)?;
            mmu.mem_write_bytes(statbuf, file.stat()?.encode().as_slice())?;
            Ok(0)
        } else {
            let filename = read_osstring!(mmu, filename)?;
            debug!("newfstatat: {filename:?}");
            let fs = self.filesystem.borrow_mut();
            let entry = fs.resolve_last(Path::new(&filename))?;
            mmu.mem_write_bytes(statbuf, fs.stat(entry)?.encode().as_slice())?;
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
        handle: Box<dyn Handle<Endianness>>,
    ) {
        self.handles.add_replacement_handle(fd, handle);
    }
}

const AT_FDCWD: u64 = -100i32 as u64; /* Special value used to indicate the *at functions should use the current working directory. */

bitflags! {
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
