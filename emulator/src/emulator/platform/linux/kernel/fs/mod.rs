use std::cell::RefMut;
use std::os::unix::ffi::OsStrExt;

use bitflags::{
    bitflags,
    Flags,
};
use byteorder::ByteOrder;
use libc::dirname;
use mogi_derive::unfinished;
use mogi_system::components::cpu::CPU;
use num_enum::TryFromPrimitive;

use crate::emulator::abstractions::file_descriptors::FileDescriptorError;
use crate::emulator::abstractions::filesystem::FileId;
use crate::emulator::macros::*;
use crate::emulator::platform::linux::fd::{
    FileDescriptorData,
    FileDescriptorFlags,
    FD,
};
use crate::emulator::platform::linux::kernel::error::SyscallError::KernelError;
use crate::emulator::platform::linux::kernel::fs::fd_ops::FileOps;
use crate::emulator::platform::linux::kernel::*;
use crate::emulator::system::fd_map::error::FileDescriptorMapError;
use crate::emulator::system::fd_map::{
    FileDescriptor,
    FileDescriptorMap,
    InternalFD,
};
use crate::emulator::system::filesystem::{
    Filesystem,
    Stat,
};
use crate::emulator::system::memory::rw_interface::{
    DirectMemoryWrite,
    MemoryWrite,
};

pub(super) mod fd_ops;

impl<Endianness, Arch, CpuT, MmuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, MmuT, TcpT>
where
    Endianness: ByteOrder + 'static,
    Arch: Architecture,
    CpuT: CPU,
    MmuT: MMU,
    TcpT: TCPSystem,
{
    /// Write syscall. See `man write` for details
    pub fn write(
        &mut self,
        fd: FD,
        buf: EVAddr,
        size: u64,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        trace!(
            "Process write {:?}",
            str::from_utf8(
                &memory
                    .accessor(&mut mmu, self.task_id)
                    .read_bytes(buf, size as usize)?
            )
        );
        file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .map_or(Err(LinuxKernelError::EINVAL.into()), |mut fd| {
                Ok(fd
                    .write(
                        memory
                            .accessor(&mut mmu, self.task_id)
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
        fd: FD,
        iovec_ptr: EVAddr,
        iovcnt: u64,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut mmu, self.task_id);

        file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .map_or(Err(LinuxKernelError::EINVAL.into()), |mut fd| {
                let mut ret: usize = 0;
                let iovec = memory
                    .read_bytes(iovec_ptr, (iovcnt * 16) as usize)
                    .unwrap();
                for i in 0..iovcnt as usize {
                    let ptr = EVAddr::from(Endianness::read_u64(&iovec[(i * 16)..((i * 16) + 8)]));
                    let size = Endianness::read_u64(&iovec[i * 16 + 8..i * 16 + 16]) as usize;
                    let data = memory.read_bytes(ptr, size)?;
                    let written: usize = fd.write(data.as_ref())?;
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
        fd: FD,
        buf: EVAddr,
        count: u64,
        pos: u64,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let fd = &mut *fd;
        let d = match fd.pread(pos, count)? {
            None => return Err(SyscallError::Yield),
            Some(data) => data,
        };
        Ok(memory.write_direct(buf, d.as_slice())? as u64)
    }

    pub fn read(
        &mut self,
        fd: FD,
        buf: EVAddr,
        count: u64,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        let mut file = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EINVAL)?;
        let file = &mut *file;
        let data = match file.read(count)? {
            None => return Err(SyscallError::Yield),
            Some(data) => data,
        };
        Ok(memory.write_direct(buf, data.as_slice())? as u64)
    }

    pub fn sendfile(
        &mut self,
        out_fd: FD,
        in_fd: FD,
        offset: u64,
        count: u64,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        let mut in_file = file_descriptor_map
            .get_mut(&self.task_id, &in_fd)
            .ok_or(LinuxKernelError::EINVAL)?;
        let data = match in_file.read(count)? {
            None => return Err(SyscallError::Yield),
            Some(data) => data,
        };
        drop(file_descriptor_map);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut out_file = file_descriptor_map
            .get_mut(&self.task_id, &out_fd)
            .ok_or(LinuxKernelError::EINVAL)?;
        Ok(out_file
            .write(data.as_slice())
            .unwrap_or(!0)
            .try_into()
            .unwrap())
    }

    pub fn fstat(
        &mut self,
        fd: FD,
        statbuf: EVAddr<Stat>,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        let mut file = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let file = &mut *file;
        memory.write_ref(statbuf, file.stat()?)?;
        Ok(0)
    }

    pub fn stat(
        &mut self,
        filename: EVAddr,
        statbuf: EVAddr<Stat>,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        let fname = read_osstring!(memory, filename)?;
        trace!("stat file {:?}", fname);
        let fs = sys_component!(self, filesystem);
        let file = fs.resolve_last(Path::new(&fname), false)?; //TODO symlinks
        memory.write_ref(statbuf, fs.stat(file)?)?;
        Ok(0)
    }

    #[unfinished]
    pub fn openat(
        &mut self,
        _dfd: FD,
        filename: EVAddr,
        _flags: u64,
        _mode: u64,
    ) -> SyscallResult {
        // TODO: Resepct flags and for example check O_NOFOLLOW
        // TODO: Need to actually use dfd
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut mmu, self.task_id);

        let fname = read_osstring!(memory, filename)?;
        trace!("openat file: {:?}", fname);
        let file = sys_component!(self, filesystem).resolve_last(Path::new(&fname), true)?; //TODO symlinks
        let mut file_descriptor = FileDescriptor::new(Box::new(FileOps::new()));
        file_descriptor.data.file =
            Some(FileData::new(self.system.filesystem.clone(), file, Some(PathBuf::from(fname))));
        Ok(sys_component_mut!(self, fd_map)
            .insert(&self.task_id, file_descriptor)?
            .into())
    }

    /// Opens the supplied file and associates an internal file descriptor with it.
    /// This can be used for kernel internal file operations when the fd does not
    /// have to be passed to user space.
    pub fn open_internal(
        &mut self,
        filename: &Path,
    ) -> Result<InternalFD, LinuxKernelError> {
        trace!("open internal {:?}", filename);
        let file = sys_component!(self, filesystem).resolve_last(filename, true)?;
        let mut file_descriptor = FileDescriptor::new(Box::new(FileOps::new()));
        file_descriptor.data.file =
            Some(FileData::new(self.system.filesystem.clone(), file, Some(filename.to_path_buf())));
        let fd = sys_component_mut!(self, fd_map).insert_internal(&self.task_id, file_descriptor)?;
        trace!("open internal result: {:?}", fd);
        Ok(fd)
    }

    pub fn statfs(
        &mut self,
        path: EVAddr,
        buf: EVAddr,
    ) -> SyscallResult {
        todo!()
    }

    pub fn newfstatat(
        &mut self,
        dfd: FD,
        filename: EVAddr,
        statbuf: EVAddr<Stat>,
        flag: u64,
    ) -> SyscallResult {
        if statbuf.is_null() {
            Err(LinuxKernelError::EINVAL)?
        }
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut mmu, self.task_id);

        let flags = at_flags::from_bits(flag).ok_or(LinuxKernelError::EINVAL)?;
        if flags.contains(at_flags::AT_EMPTY_PATH) {
            let mut fd = file_descriptor_map
                .get_mut(&self.task_id, &dfd)
                .ok_or(LinuxKernelError::EBADFD)?;
            memory.write_ref(statbuf, fd.stat()?)?;
            Ok(0)
        } else {
            let filename = read_osstring!(memory, filename)?;
            let path = Path::new(&filename);
            let dir = if !path.is_absolute() {
                Some(get_dir_fse_id(file_descriptor_map, &self.task_id, &dfd)?)
            } else {
                None
            };
            let fs = sys_component_mut!(self, filesystem);
            let entry = fs.resolve_last_relative(dir, path, false)?; //TODO symlinks
            memory.write_ref(statbuf, fs.stat(entry)?)?;
            Ok(0)
        }
    }

    pub fn readlinkat(
        &mut self,
        dirfd: FD,
        pathname: EVAddr,
        buf: EVAddr,
        bufsize: usize,
    ) -> SyscallResult {
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        Ok(memory
            .write_bounded_slice(buf, "/home/user/open62541_server_release".as_bytes(), bufsize)?
            .0 as u64)
    }

    pub fn close(
        &mut self,
        fd: FD,
    ) -> SyscallResult {
        Ok(sys_component_mut!(self, fd_map)
            .remove(&self.task_id, &fd)
            .map(|_| 0)?)
    }

    pub fn dup(
        &mut self,
        fd: FD,
    ) -> SyscallResult {
        Ok(sys_component_mut!(self, fd_map)
            .dup(&self.task_id, fd)?
            .into())
    }

    pub fn dup2(
        &mut self,
        fd: FD,
        newfd: FD,
    ) -> SyscallResult {
        if fd == newfd {
            return Ok(newfd.into());
        }

        Ok(sys_component_mut!(self, fd_map)
            .dup2(&self.task_id, fd, newfd)?
            .into())
    }

    pub fn dup3(
        &mut self,
        fd: FD,
        newfd: FD,
        flags: u64,
    ) -> SyscallResult {
        if fd == newfd {
            Err(LinuxKernelError::EINVAL)?;
        }

        Ok(sys_component_mut!(self, fd_map)
            .dup3(&self.task_id, fd, newfd, flags)
            .map_err(|_| LinuxKernelError::EBADF)?
            .into())
    }

    pub fn getcwd(
        &self,
        buf: EVAddr,
        size: u64,
    ) -> SyscallResult {
        let mut memory = sys_component_mut!(self, memory);
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        let buf_content = read_osstring!(memory, buf)?;
        let cwd_fd = sys_component_mut!(self, fd_map).get_cwd(&self.task_id)?;
        let cwd_path = cwd_fd.data.file.unwrap().path.unwrap();
        if cwd_path.as_os_str().len() as u64 > size {
            Err(SyscallError::KernelError(LinuxKernelError::EINVAL))
        } else {
            memory.write_bytes(buf, format!("{}\x00", cwd_path.as_os_str().to_str().unwrap()).as_bytes());
            Ok(buf.into())
        }
    }

    pub fn lseek(
        &self,
        fd: FD,
        offset: u64,
        whence: Whence,
    ) -> SyscallResult {
        let mut fd_map = sys_component_mut!(self, fd_map);
        let file = fd_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;

        let new_pos = if file.data.stdio.is_some() {
            let mut stdio = sys_component_mut!(self, stdio);
            stdio.lseek(whence, offset as i64)
        } else if file.data.file.is_some() {
            file.data
                .file
                .as_mut()
                .unwrap()
                .lseek(whence, offset as i64)
        } else {
            unimplemented!()
        };
        Ok(new_pos as u64)
    }

    pub fn chdir(
        &self,
        path: EVAddr,
    ) -> SyscallResult {
        let mut memory = sys_component_mut!(self, memory);
        let mut mmu = sys_component_mut!(self, mmu);
        let mut filesystem = sys_component_mut!(self, filesystem);
        let mut memory = memory.accessor(&mut mmu, self.task_id);
        let mut fd_map = sys_component_mut!(self, fd_map);

        let pathname = read_osstring!(memory, path)?;
        let dir = filesystem.resolve_last(Path::new(&pathname), true)?;
        let mut cwd_fd = FileDescriptor::new(Box::new(FileOps::new()));
        cwd_fd.data.file = Some(FileData::new(self.system.filesystem.clone(), dir, Some(PathBuf::from(&pathname))));
        drop(filesystem);
        if !cwd_fd.stat()?.S_ISDIR() {
            return Err(SyscallError::KernelError(LinuxKernelError::ENOTDIR));
        }
        fd_map.insert_cwd(&self.task_id, cwd_fd)?;
        Ok(0)
    }

    pub fn chdir_external(
        &self,
        path: PathBuf,
    ) -> SyscallResult {
        let dir = sys_component_mut!(self, filesystem).resolve_last(Path::new(&path), true)?;
        let mut cwd_fd = FileDescriptor::new(Box::new(FileOps::new()));
        cwd_fd.data.file = Some(FileData::new(self.system.filesystem.clone(), dir, Some(PathBuf::from(&path))));
        if !cwd_fd.stat()?.S_ISDIR() {
            return Err(SyscallError::KernelError(LinuxKernelError::ENOTDIR));
        }
        sys_component_mut!(self, fd_map).insert_cwd(&self.task_id, cwd_fd)?;
        Ok(0)
    }

    pub fn fchdir(
        &self,
        fd: FD,
    ) -> SyscallResult {
        let fd_map = sys_component_mut!(self, fd_map);
        let cwd_fd = fd_map.get(&self.task_id, &fd);
        if cwd_fd.is_none() {
            return Err(SyscallError::KernelError(LinuxKernelError::ENOENT));
        }
        let cwd_fd = cwd_fd.unwrap().to_owned();
        drop(fd_map);
        if !cwd_fd.to_owned().stat()?.S_ISDIR() {
            return Err(SyscallError::KernelError(LinuxKernelError::ENOTDIR));
        }
        let mut fd_map = sys_component_mut!(self, fd_map);
        fd_map.insert_cwd(&self.task_id, cwd_fd);
        Ok(0)
    }
}

/// Get the fs entry id for the supplied file descriptor if it is a directory.
fn get_dir_fse_id(
    mut map: RefMut<'_, FileDescriptorMap<FD, TaskId, FileDescriptorData>>,
    task_id: &TaskId,
    fd: &FD,
) -> Result<FileId, LinuxKernelError> {
    let mut fd = map.get_mut(task_id, fd).ok_or(LinuxKernelError::EBADFD)?;
    let fd = &mut *fd;
    let is_dir = fd.stat()?.S_ISDIR();
    if is_dir {
        fd.data
            .file
            .as_ref()
            .ok_or(LinuxKernelError::ENOTDIR)
            .map(|d| d.fse_id)
    } else {
        Err(LinuxKernelError::ENOTDIR)
    }
}

#[derive(Clone)]
pub struct FileData {
    filesystem: Rc<RefCell<Filesystem>>,
    fse_id: FileId,
    pub pos: usize,
    /// An optional path buf that was used to open this file object.
    path: Option<PathBuf>,
}

impl FileData {
    pub fn lseek(
        &mut self,
        whence: Whence,
        offset: i64,
    ) -> usize {
        match whence {
            Whence::SEEK_SET => self.pos = offset as usize,
            Whence::SEEK_CUR => {
                self.pos = self.pos.wrapping_add_signed(offset as isize);
            }
            Whence::SEEK_END => {
                self.pos = self
                    .filesystem
                    .borrow_mut()
                    .read_all(self.fse_id)
                    .expect("file should exist, else fd cannot be open")
                    .len();
                self.pos = self.pos.wrapping_add_signed(offset as isize);
            }
        }
        self.pos
    }
}

impl Debug for FileData {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("FileData")
            .field("fse_id", &self.fse_id)
            .field("pos", &self.pos)
            .finish()
    }
}

impl FileData {
    pub fn new(
        filesystem: Rc<RefCell<Filesystem>>,
        fse_id: FileId,
        path: Option<PathBuf>,
    ) -> FileData {
        FileData {
            filesystem,
            fse_id,
            pos: 0,
            path,
        }
    }

    pub fn get_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

#[derive(Debug, Copy, Clone, TryFromPrimitive)]
#[repr(u64)]
#[allow(non_camel_case_types)]
pub enum Whence {
    SEEK_SET,
    SEEK_CUR,
    SEEK_END,
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
