use bitflags::{bitflags, Flags};
use byteorder::ByteOrder;

use crate::emulator::macros::*;
use crate::emulator::platform::linux::fd::{FD, FileDescriptorData};
use crate::emulator::platform::linux::kernel::fs::fd_ops::FileOps;
use crate::emulator::platform::linux::kernel::*;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::FSEntryId;
use crate::emulator::system::memory::rw_interface::{DirectMemoryWrite, MemoryWrite};

mod fd_ops;

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    /// Write syscall. See `man write` for details
    pub fn write(
        &mut self,
        mmu: &mut impl MMU,
        fd: FD,
        buf: EVAddr,
        size: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .map_or(Err(LinuxKernelError::EINVAL), |mut fd| {
                let fd = &mut *fd;
                Ok(fd
                    .ops
                    .write(
                        &mut fd.data,
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
        fd: FD,
        iovec_ptr: EVAddr,
        iovcnt: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .map_or(Err(LinuxKernelError::EINVAL), |mut fd| {
                let fd = &mut *fd;
                let mut ret: usize = 0;
                let iovec = memory
                    .read_bytes(iovec_ptr, (iovcnt * 16) as usize)
                    .unwrap();
                for i in 0..iovcnt as usize {
                    let ptr = EVAddr::from(Endianness::read_u64(&iovec[(i * 16)..((i * 16) + 8)]));
                    let size = Endianness::read_u64(&iovec[i * 16 + 8..i * 16 + 16]) as usize;
                    let data = memory.read_bytes(ptr, size)?;
                    let written: usize = fd.ops.write(&mut fd.data, data.as_ref())?;
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
        fd: FD,
        buf: EVAddr,
        count: u64,
        pos: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let fd = &mut *fd;
        let d = fd.ops.pread(&mut fd.data, pos, count).unwrap();
        Ok(memory.write_direct(buf, d.as_slice())? as u64)
    }

    pub fn read(
        &mut self,
        mmu: &mut impl MMU,
        fd: FD,
        buf: EVAddr,
        count: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let mut file = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EINVAL)?;
        let file = &mut *file;
        Ok(memory.write_direct(buf, file.ops.read(&mut file.data, count).unwrap().as_slice())? as u64)
    }

    pub fn fstat(
        &mut self,
        mmu: &mut impl MMU,
        fd: FD,
        statbuf: EVAddr,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let mut file = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let file = &mut *file;
        memory.write(statbuf, file.ops.stat(&mut file.data)?)?;
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
        _dfd: FD,
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
        let mut file_descriptor = FileDescriptor::new(Box::new(FileOps::new(self.system.clone())));
        file_descriptor.data.file = Some(FileData::new(file));
        Ok(self
            .system
            .fd_map
            .borrow_mut()
            .insert(&self.task_id, file_descriptor)?
            .into())
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
        dfd: FD,
        filename: EVAddr,
        statbuf: EVAddr,
        flag: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        let flags = at_flags::from_bits(flag).ok_or(LinuxKernelError::EINVAL)?;
        if flags.contains(at_flags::AT_EMPTY_PATH) {
            let mut fd = file_descriptor_map
                .get_mut(&self.task_id, &dfd)
                .ok_or(LinuxKernelError::EBADFD)?;
            let fd = &mut *fd;
            memory.write(statbuf, fd.ops.stat(&mut fd.data)?)?;
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
        fd: FD,
    ) -> SyscallResult {
        Ok(self
            .system
            .fd_map
            .borrow_mut()
            .remove(&self.task_id, &fd)
            .map(|_| 0)?)
    }

    pub fn add_replacement_handle(
        &mut self,
        fd: FD,
        handle: Box<dyn FileDescriptorOps<FileDescriptorData>>,
    ) {
        todo!()
        // self.fd_map.add_replacement_handle(fd, handle);
    }
}

#[derive(Debug, Clone)]
pub struct FileData {
    fse_id: FSEntryId,
    pos: usize,
}

impl FileData {
    pub fn new(fse_id: FSEntryId) -> FileData {
        FileData { fse_id, pos: 0 }
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
