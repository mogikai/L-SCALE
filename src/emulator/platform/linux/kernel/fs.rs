use byteorder::ByteOrder;

use crate::emulator::platform::linux::kernel::*;

impl<Endianness: ByteOrder + 'static, Arch: Architecture>
LinuxKernel<Endianness, Arch> {
    pub fn write(&mut self, _: &mut impl CPU, fd: u64, buf: u64, size: u64) -> SyscallResult {
        self.handles.get_mut(&fd).map_or(Err(LinuxKernelError::EINVAL), |handle| {
            let data = self.memory.read_internal(buf, size as usize).unwrap();
            Ok(handle.write(&*data).unwrap_or(!0).try_into().unwrap())
        })
    }

    pub fn writev(&mut self, _: &mut impl CPU, fd: u64, iovec_ptr: Vaddr, iovcnt: u64) -> SyscallResult {
        self.handles.get_mut(&fd).map_or(Err(LinuxKernelError::EINVAL), |handle| {
            let mut ret = 0;
            let iovec = self.memory.read_internal(iovec_ptr, (iovcnt * 16) as usize).unwrap();
            for i in 0..iovcnt as usize {
                let ptr = Endianness::read_u64(&iovec[(i * 16)..((i * 16) + 8)]);
                let size = Endianness::read_u64(&iovec[i * 16 + 8..i * 16 + 16]) as usize;
                let data = self.memory.read_internal(ptr, size)?;
                let written = handle.write(data.as_ref())?;
                ret += written;
                // Early return if not all bytes were written. We need to stop writing in this case
                if written != data.len() { return Ok(ret.try_into().unwrap()); };
            }
            Ok(ret.try_into().unwrap())
        })
    }

    pub fn pread64(&mut self,
                   _: &mut impl CPU,
                   fd: u64,
                   buf: u64,
                   count: u64,
                   pos: u64)
        -> SyscallResult {
        let file = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADF)?;
        let d = file.pread(pos, count).unwrap();
        Ok(self.memory.write(buf, d.as_slice()))
    }

    pub fn read(&mut self,
                _: &mut impl CPU,
                fd: u64,
                buf: u64,
                count: u64)
        -> SyscallResult {
        if let Some(f) = self.handles.get_mut(&fd) {
            Ok(self.memory.write(buf, f.read(count).unwrap().as_slice()))
        } else {
            Err(LinuxKernelError::EINVAL)
        }
    }

    pub fn fstat(&mut self,
                 _: &mut impl CPU,
                 fd: u64,
                 statbuf: Vaddr)
        -> SyscallResult {
        let file = self.handles.get_mut(&fd).ok_or(LinuxKernelError::EBADF)?;
        self.memory.write_direct(statbuf, &*file.stat()?)?;
        Ok(0)
    }

    pub fn stat(&mut self,
                _: &mut impl CPU,
                filename: u64,
                statbuf: Vaddr)
        -> SyscallResult {
        let fname = read_osstring!(self.memory, filename).unwrap();
        let fs = self.filesystem.borrow();
        let file = fs.resolve_last(Path::new(&fname))?;
        let stat = fs.stat(file)?;
        self.memory.write_direct(statbuf, stat)?;
        Ok(0)
    }

    pub fn openat(&mut self,
                  _: &mut impl CPU,
                  _dfd: u64,
                  filename: u64,
                  _flags: u64,
                  _mode: u64)
        -> SyscallResult {
        let fname = read_osstring!(self.memory, filename).unwrap();
        trace!("openat file: {:?}", fname);
        let p = Path::new(&fname);
        let file = self.filesystem.borrow().resolve_last(p)?;
        let fd = *self.handles.keys().max().unwrap_or(&2) + 1;
        self.handles.insert(fd, Box::new(FileHandle::new(self.filesystem.clone(), file)));
        Ok(fd)
    }

    pub fn close(&mut self,
                 _: &mut impl CPU,
                 fd: u64)
        -> SyscallResult {
        self.handles.remove(&fd);
        Ok(0)
    }

    pub fn add_replacement_handle(&mut self,
                                  fd: u64,
                                  handle: Box<dyn Handle<Endianness>>) {
        self.handles.add_replacement_handle(fd, handle);
    }
}