use crate::emulator::abstractions::file_descriptors::FileDescriptorError;
use bitflags::{bitflags, Flags};
use byteorder::ByteOrder;
use std::cell::RefMut;

use crate::emulator::macros::*;
use crate::emulator::platform::linux::fd::{FileDescriptorData, FD};
use crate::emulator::platform::linux::kernel::fs::fd_ops::FileOps;
use crate::emulator::platform::linux::kernel::*;
use crate::emulator::system::fd_map::{FileDescriptor, FileDescriptorMap, InternalFD};
use crate::emulator::system::filesystem::{FSEntryId, Filesystem};
use crate::emulator::system::memory::rw_interface::{DirectMemoryWrite, MemoryWrite};

mod fd_ops;

impl<Endianness, Arch, CpuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder + 'static,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    /// Write syscall. See `man write` for details
    pub fn write(
        &mut self,
        fd: FD,
        buf: EVAddr,
        size: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .map_or(Err(LinuxKernelError::EINVAL.into()), |mut fd| {
                Ok(fd
                    .write(
                        memory
                            .accessor(&mut *cpu, self.task_id)
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
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let memory = memory.accessor(&mut *cpu, self.task_id);
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
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut *cpu, self.task_id);
        let mut fd = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let fd = &mut *fd;
        let d = fd.pread(pos, count).unwrap();
        Ok(memory.write_direct(buf, d.as_slice())? as u64)
    }

    pub fn read(
        &mut self,
        fd: FD,
        buf: EVAddr,
        count: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut *cpu, self.task_id);
        let mut file = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EINVAL)?;
        let file = &mut *file;
        Ok(memory.write_direct(buf, file.read(count).unwrap().as_slice())? as u64)
    }

    pub fn fstat(
        &mut self,
        fd: FD,
        statbuf: EVAddr,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut *cpu, self.task_id);
        let mut file = file_descriptor_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let file = &mut *file;
        memory.write(statbuf, file.stat()?)?;
        Ok(0)
    }

    pub fn stat(
        &mut self,
        filename: EVAddr,
        statbuf: EVAddr,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut *cpu, self.task_id);
        let fname = read_osstring!(memory, filename)?;
        trace!("stat file {:?}", fname);
        let fs = sys_component!(self, filesystem);
        let file = fs.resolve_last(Path::new(&fname), false)?; //TODO symlinks
        memory.write(statbuf, fs.stat(file)?)?;
        Ok(0)
    }

    pub fn openat(
        &mut self,
        _dfd: FD,
        filename: EVAddr,
        _flags: u64,
        _mode: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let memory = memory.accessor(&mut *cpu, self.task_id);
        let fname = read_osstring!(memory, filename)?;
        trace!("openat file: {:?}", fname);
        let file = sys_component!(self, filesystem).resolve_last(Path::new(&fname), false)?; //TODO symlinks
        let mut file_descriptor = FileDescriptor::new(Box::new(FileOps::new()));
        file_descriptor.data.file = Some(FileData::new(self.system.filesystem.clone(), file));
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
        file_descriptor.data.file = Some(FileData::new(self.system.filesystem.clone(), file));
        let r = sys_component_mut!(self, fd_map).insert_internal(&self.task_id, file_descriptor)?;
        trace!("open internal result: {:?}", r);
        return Ok(r);
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
        statbuf: EVAddr,
        flag: u64,
    ) -> SyscallResult {
        if statbuf == EVA_NULL {
            Err(LinuxKernelError::EINVAL)?
        }
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut memory = memory.accessor(&mut *cpu, self.task_id);

        let flags = at_flags::from_bits(flag).ok_or(LinuxKernelError::EINVAL)?;
        if flags.contains(at_flags::AT_EMPTY_PATH) {
            let mut fd = file_descriptor_map
                .get_mut(&self.task_id, &dfd)
                .ok_or(LinuxKernelError::EBADFD)?;
            memory.write(statbuf, fd.stat()?)?;
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
            memory.write(statbuf, fs.stat(entry)?)?;
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
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut *cpu, self.task_id);
        Ok(memory
            .write_bounded(buf, "/home/user/open62541_server_release".as_bytes(), bufsize)?
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

    pub fn add_replacement_handle(
        &mut self,
        fd: FD,
        handle: Box<dyn FileDescriptorOps<D = FileDescriptorData>>,
    ) {
        todo!()
        // self.fd_map.add_replacement_handle(fd, handle);
    }
}

/// Get the fs entry id for the supplied file descriptor if it is a directory.
fn get_dir_fse_id(
    map: RefMut<FileDescriptorMap<FD, TaskId, FileDescriptorData>>,
    task_id: &TaskId,
    fd: &FD,
) -> Result<FSEntryId, LinuxKernelError> {
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

#[derive(Debug, Clone)]
pub struct FileData {
    filesystem: Rc<RefCell<Filesystem>>,
    fse_id: FSEntryId,
    pos: usize,
}

impl FileData {
    pub fn new(
        filesystem: Rc<RefCell<Filesystem>>,
        fse_id: FSEntryId,
    ) -> FileData {
        FileData {
            filesystem,
            fse_id,
            pos: 0,
        }
    }

    pub fn get_path(
        &self,
        system: &Filesystem,
    ) -> Result<PathBuf, FileDescriptorError> {
        Ok(system.path_from(self.fse_id)?)
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
