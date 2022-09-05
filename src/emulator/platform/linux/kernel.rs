use std::{fs, io};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::Keys;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use byteorder::ByteOrder;
use libc;
use libc::timespec;
use linux_libc_auxv::AuxVarType;

use crate::emulator::abstractions::cpu::{CPU, CPUError, MMU, Permission, RegisterX86, Vaddr};
use crate::emulator::abstractions::handles::{FSError, Handle};
use crate::emulator::platform::linux::filedescriptors::OpenFile;
use crate::emulator::system::default_handles;
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory;
use crate::emulator::system::memory::{MappingError, Memory, MemoryWrite};
use crate::read_osstring;

const MAP_ANONYMOUS: u64 = 0x20;
const MAP_PRIVATE: u64 = 0x2;
const MAP_FIXED: u64 = 0x10;
const MAP_DENYWRITE: u64 = 0x0800;

struct HandleMap {
    handles: HashMap<u64, Box<dyn Handle>>,
    replacement_handles: HashMap<u64, VecDeque<Box<dyn Handle>>>,
}

impl HandleMap {
    fn new() -> Self {
        Self { handles: HashMap::new(), replacement_handles: HashMap::new() }
    }
}

impl HandleMap {
    /// This function is used to add a replacement handle that is inserted instead of the actual handle
    /// for the first call that is associated with the supplied fd. If multiple replacements are added
    /// for the same fd, they will be used subsequently for each new insert that happens after the fd
    /// was closed, i.e. after it was removed
    /// # Arguments
    ///
    /// * `fd`: The fd the replacement is associated with.
    /// * `handle`: The replacement handle.
    ///
    /// returns: ()
    fn add_replacement_handle(&mut self, fd: u64, handle: Box<dyn Handle>) {
        let handles = self.replacement_handles.entry(fd).or_default();
        handles.push_front(handle);
    }

    fn insert(&mut self, fd: u64, handle: Box<dyn Handle>) -> Option<Box<dyn Handle>> {
        // Insert the handle if no replacement exists. Otherwise insert the replacement
        self.handles.insert(fd, self.replacement_handles.entry(fd).or_default().pop_back().unwrap_or(handle))
    }

    fn get_mut(&mut self, fd: &u64) -> Option<&mut Box<dyn Handle>> {
        self.handles.get_mut(fd)
    }

    fn keys(&self) -> Keys<u64, Box<dyn Handle>> {
        self.handles.keys()
    }

    fn get(&self, fd: &u64) -> Option<&Box<dyn Handle>> {
        self.handles.get(fd)
    }

    fn remove(&mut self, fd: &u64) -> Option<Box<dyn Handle>> {
        self.handles.remove(fd)
    }
}

pub struct LinuxKernel<Endianness: ByteOrder> {
    memory: Memory<Endianness>,
    filesystem: Rc<RefCell<Filesystem>>,
    handles: HandleMap,
}

impl<Endianness: ByteOrder> LinuxKernel<Endianness> {
    pub fn new(memory: Memory<Endianness>, filesystem: Rc<RefCell<Filesystem>>) -> LinuxKernel<Endianness> {
        LinuxKernel { memory, filesystem, handles: HandleMap::new() }
    }

    pub fn load_binary(&mut self, cpu: &mut (impl CPU + MMU), binary_path: &Path) -> Result<(), LinuxKernelError> {
        let fs = self.filesystem.borrow();
        let root_dir = fs.on_disk_root().unwrap();
        let buffer = fs::read(getpath(root_dir, binary_path).unwrap()).unwrap();
        let elf = goblin::elf::Elf::parse(&buffer).unwrap();
        let interpreter_path = getpath(root_dir, Path::new(elf.interpreter.unwrap())).unwrap();
        let buffer = fs::read(&interpreter_path).unwrap();
        let interpreter = goblin::elf::Elf::parse(&buffer).unwrap();
        cpu.reg_write(RegisterX86::RIP.into(), interpreter.entry).unwrap();
        //TODO iterator
        for ph in interpreter.program_headers {
            if ph.p_type == goblin::elf::program_header::PT_LOAD {
                let perms = getperms(&ph);
                if ph.p_offset + ph.p_filesz > buffer.len().try_into().unwrap() {
                    return Err(LinuxKernelError::ENOEXEC);
                }
                let data: &[u8] = &buffer[ph.file_range()];
                let vm_range = ph.p_vaddr..(ph.p_vaddr + ph.p_memsz);
                info!("vm_range   0x{:X}..0x{:X}", vm_range.start, vm_range.end);
                info!("file_range 0x{:X}..0x{:X}", ph.file_range().start, ph.file_range().end);
                self.memory.map_data(data, vm_range, perms, &mut *cpu).unwrap();
            }
        }
        drop(fs);
        self.initstack(cpu, binary_path, interpreter_path.as_path(), interpreter.entry)
    }

    fn initstack(&mut self, cpu: &mut impl CPU, binary_path: &Path, interpreter_path: &Path, entrypoint: Vaddr) -> Result<(), LinuxKernelError> {
        info!("Initializing stack");
        cpu.reg_write(RegisterX86::RSP.into(), 0x4000000000000000).expect("failed to write RSP");
        let mem = &mut self.memory;
        let mut env: Vec<u64> = Vec::new();
        let mut argv: Vec<u64> = Vec::new();
        let mut aux: Vec<(u64, u64)> = Vec::new();
        crate::push_stack!(mem, 0u64, RegisterX86::RSP.into(), cpu);
        env.push(crate::push_stack!(mem, "SHELL=/bin/sh", RegisterX86::RSP.into(), cpu));
        env.push(crate::push_stack!(mem, "LD_DEBUG=all", RegisterX86::RSP.into(), cpu));
        env.push(0);
        argv.push(2);
        argv.push(crate::push_stack!(mem, interpreter_path.to_str().unwrap(), RegisterX86::RSP.into(), cpu));
        argv.push(crate::push_stack!(mem, binary_path.to_str().unwrap(), RegisterX86::RSP.into(), cpu));
        argv.push(0);
        aux.push((AuxVarType::Pagesz as u64, 4096));
        aux.push((AuxVarType::HwCap as u64, 0xbfebfbff));
        aux.push((AuxVarType::Uid as u64, 1));
        aux.push((AuxVarType::Gid as u64, 1));
        aux.push((AuxVarType::EUid as u64, 1));
        aux.push((AuxVarType::EGid as u64, 1));
        aux.push((AuxVarType::Platform as u64, crate::push_stack!(mem, "x86_64", RegisterX86::RSP.into(), cpu)));
        aux.push((AuxVarType::Entry as u64, entrypoint));
        aux.push((AuxVarType::Phdr as u64, 56));
        aux.push((AuxVarType::Phent as u64, 11));
        aux.push((AuxVarType::Secure as u64, 0));
        aux.push((0, 0));

        // align stack
        let rsp = cpu.reg_read(RegisterX86::RSP.into()).expect("error can't read RSP");
        if (rsp & !0xf) != rsp {
            cpu.reg_write(RegisterX86::RSP.into(), rsp & !0xf).unwrap();
        }

        crate::push_stack!(mem, &aux, RegisterX86::RSP.into(), cpu);
        crate::push_stack!(mem, &env, RegisterX86::RSP.into(), cpu);
        crate::push_stack!(mem, &argv, RegisterX86::RSP.into(), cpu);
        Ok(())
    }

    pub fn add_stdio_handles(&mut self) {
        self.handles.insert(0, Box::new(default_handles::ReadOnly::new()));
        self.handles.insert(1, Box::new(default_handles::WriteOnly::new()));
        self.handles.insert(2, Box::new(default_handles::WriteOnly::new()));
    }

    pub fn add_replacement_handle(&mut self, fd: u64, handle: Box<dyn Handle>) {
        self.handles.add_replacement_handle(fd, handle);
    }

    pub fn brk(&mut self, cpu: &mut impl MMU, addr: u64) -> u64 {
        if addr != 0 {
            self.memory.realloc_heap(addr, cpu);
        }
        self.memory.heapend()
    }

    pub fn arch_prctl(&mut self, cpu: &mut impl CPU, code: u64, addr: u64) -> u64 {
        match code {
            0x1001 => { //ARCH_SET_GS
                !0
            }
            0x1002 => { //ARCH_SET_FS
                let fsbytes = self.memory.read_internal(addr, 8).unwrap();
                let fs = Endianness::read_u64(&fsbytes);
                cpu.reg_write(RegisterX86::FS_BASE.into(), fs).unwrap();
                0
            }
            0x1003 => !0, //ARCH_GET_FS
            0x1004 => !0, //ARCH_GET_GS
            _ => !0,
        }
    }

    pub fn clock_gettime(&mut self, cpu: &mut impl MMU, clk_id: u64, tp: Vaddr) -> u64 {
        let ts = timespec { tv_sec: 0, tv_nsec: 0 };
        self.memory.write_struct(tp, ts).unwrap();
        // TODO: Implement actual timer
        0
    }

    pub fn write(&mut self, _: &mut impl CPU, fd: u64, buf: u64, size: u64) -> u64 {
        self.handles.get_mut(&fd).map_or(!0, |handle| {
            let data = self.memory.read_internal(buf, size as usize).unwrap();
            handle.write(&*data).unwrap_or(!0).try_into().unwrap()
        })
    }

    pub fn writev(&mut self, _: &mut impl CPU, fd: u64, iovec_ptr: Vaddr, iovcnt: u64) -> u64 {
        self.handles.get_mut(&fd).map_or(!0, |handle| {
            let mut ret = 0;
            let iovec = self.memory.read_internal(iovec_ptr, (iovcnt * 16) as usize).unwrap();
            for i in 0..iovcnt as usize {
                let ptr = Endianness::read_u64(&iovec[(i * 16)..((i * 16) + 8)]);
                let size = Endianness::read_u64(&iovec[i * 16 + 8..i * 16 + 16]) as usize;
                let data = self.memory.read_internal(ptr, size).unwrap();
                match handle.write(data.as_ref()) {
                    Ok(written) => {
                        ret += written;
                        // Early return if not all bytes were written. We need to stop writing in this case
                        if written != data.len() { return ret.try_into().unwrap(); };
                    }
                    Err(_e) => return !0,
                };
            }
            ret.try_into().unwrap()
        })
    }
    pub fn openat(&mut self, _: &mut impl CPU, _dfd: u64, filename: u64, _flags: u64, _mode: u64) -> u64 {
        let fname = read_osstring!(self.memory, filename).unwrap();
        let p = Path::new(&fname);
        if let Ok(file) = self.filesystem.borrow().resolve_last(p) {
            let fd: u64 = *self.handles.keys().max().unwrap_or(&2) + 1;
            self.handles.insert(fd, Box::new(OpenFile::new(self.filesystem.clone(), file)));
            return fd;
        } else {
            // TODO: Do correct error mapping
            LinuxKernelError::EINVAL.into()
        }
    }
    pub fn read(&mut self, _: &mut impl CPU, fd: u64, buf: u64, count: u64) -> u64 {
        if let Some(f) = self.handles.get_mut(&fd) {
            self.memory.write(buf, f.read(count).unwrap().as_slice())
        } else {
            !0
        }
    }
    pub fn getpid(&mut self, _: &mut impl CPU) -> u64 {
        // TODO: Implement multi threading / processing
        1
    }
    fn mmap_anon(&mut self, cpu: &mut impl MMU, addr: Vaddr, length: u64, prot: u64, flags: u64) -> Result<u64, LinuxKernelError> {
        let addr = if (flags & MAP_FIXED) != 0 {
            if let Err(e) = self.memory.unmap(addr, length, cpu) {
                return Err(match e {
                    MappingError::NotAligned => LinuxKernelError::EINVAL,
                    MappingError::ZeroSize => LinuxKernelError::EINVAL,
                    _ => LinuxKernelError::EINVAL
                });
            }
            addr
        } else {
            //self.memory.find_free(addr, length)
            addr
        };
        let addr = match self.memory.mmap(addr, length, prot.try_into()?, cpu) {
            Ok(addr) => addr,
            Err(e) => {
                println!("mmap error {:?}", e);
                return Err(LinuxKernelError::EINVAL);
            }
        };
        Ok(addr)
    }
    fn mmap_private(&mut self, cpu: &mut impl MMU, addr: u64, length: u64, prot: u64, flags: u64, fd: u64, offset: u64) -> u64 {
        if fd >= 1 << 31 {
            return !0;
        }
        let addr = self.mmap_anon(cpu, addr, length, prot, flags).unwrap();
        if addr == !0 {
            return !0;
        }
        let f = match self.handles.get(&fd) {
            Some(f) => f,
            _ => return !0,
        };
        let data = match f.pread(offset, length) {
            Ok(d) => d,
            _ => return !0,
        };
        self.memory.write(addr, data.as_slice());
        addr
    }
    pub fn mmap(&mut self, cpu: &mut impl MMU, addr: Vaddr, length: u64, prot: u64, flags: u64, fd: u64, offset: u64) -> u64 {
        if (flags & !(MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED | MAP_DENYWRITE)) != 0 {
            return !0;
        }
        if (flags & MAP_ANONYMOUS) != 0 {
            return self.mmap_anon(cpu, addr, length, prot, flags).unwrap_or_else(|e| e.into());
        }
        if (flags & MAP_PRIVATE) != 0 {
            return self.mmap_private(cpu, addr, length, prot, flags, fd, offset);
        }
        !0
    }
    pub fn close(&mut self, _: &mut impl CPU, fd: u64) -> u64 {
        self.handles.remove(&fd);
        0
    }
    pub fn uname(&mut self, _: &mut impl CPU, buf: u64) -> u64 {
        //TODO check if address is mapped and has space for struct new_utsname
        self.memory.write(buf + 0 * 65, "Linux");
        self.memory.write(buf + 1 * 65, "ILT792");
        self.memory.write(buf + 2 * 65, "5.4.0-110-generic");
        self.memory.write(buf + 3 * 65, "#124-Ubuntu SMP Thu Apr 14 19:46:19 UTC 2022");
        self.memory.write(buf + 4 * 65, "x86_64");
        self.memory.write(buf + 5 * 65, "(none)");
        0
    }
    pub fn gettimeofday(&mut self, _: &mut impl CPU, tv: Vaddr, tz: Vaddr) -> u64 {
        if tv == 0 {
            return LinuxKernelError::EINVAL.into();
        }
        if tz != 0 {
            todo!("Timezone not implemented yet")
        }
        let timeval = libc::timeval { tv_sec: 0, tv_usec: 0 };
        // TODO: Implement actual timer
        self.memory.write_struct(tv, timeval).unwrap();
        0
    }
    pub fn access(&mut self, _: &mut impl CPU, _filename: u64, _mode: u64) -> u64 {
        !0
    }
    pub fn fstat(&mut self, _: &mut impl CPU, fd: u64, statbuf: Vaddr) -> u64 {
        let file = self.handles.get_mut(&fd);
        match file {
            Some(f) => {
                self.memory.write(statbuf, &f.stat());
                return 0;
            }
            None => {}
        }
        !0
    }
    pub fn stat(&mut self, _: &mut impl CPU, filename: u64, statbuf: Vaddr) -> u64 {
        let fname = read_osstring!(self.memory, filename).unwrap();
        let file = self.filesystem.borrow().resolve_last(Path::new(&fname));
        if let Ok(file) = file {
            match self.filesystem.borrow().stat(file) {
                Ok(s) => {
                    let s: Vec<u8> = s.into();
                    self.memory.write(statbuf, s.as_slice());
                    0
                }
                Err(_) => !0,
            }
        } else {
            !0
        }
    }
    pub fn pread64(&mut self, _: &mut impl CPU, fd: u64, buf: u64, count: u64, pos: u64) -> u64 {
        let file = self.handles.get_mut(&fd);
        if let Some(f) = file {
            let d = f.pread(pos, count).unwrap();
            self.memory.write(buf, d.as_slice())
        } else {
            !0
        }
    }
    pub fn mprotect(&mut self, cpu: &mut impl MMU, start: u64, len: u64, prot: u64) -> u64 {
        self.memory.protect(start, len, prot.try_into().unwrap(), cpu);
        0
    }
}


fn getpath(dir: &Path, file: &Path) -> Result<PathBuf, io::Error> {
    let p = if file.is_absolute() {
        file.strip_prefix("/").unwrap()
    } else {
        file
    };
    let dir = dir.canonicalize()?;
    let p = dir.join(p).canonicalize()?;
    if p.starts_with(dir) { Ok(p) } else { Err(io::ErrorKind::NotFound.into()) }
}

fn getperms(ph: &goblin::elf::program_header::ProgramHeader) -> Permission {
    let mut perms = Permission::NONE;
    if ph.is_read() {
        perms |= Permission::READ;
    }
    if ph.is_write() {
        perms |= Permission::WRITE;
    }
    if ph.is_executable() {
        perms |= Permission::EXEC;
    }
    perms
}

#[derive(Debug)]
#[repr(i64)]
pub enum LinuxKernelError {
    EPERM = -1,
    ENOENT = -2,
    ESRCH = -3,
    EINTR = -4,
    EIO = -5,
    ENXIO = -6,
    E2BIG = -7,
    ENOEXEC = -8,
    EBADF = -9,
    ECHILD = -10,
    EAGAIN = -11,
    ENOMEM = -12,
    EACCES = -13,
    EFAULT = -14,
    ENOTBLK = -15,
    EBUSY = -16,
    EEXIST = -17,
    EXDEV = -18,
    ENODEV = -19,
    ENOTDIR = -20,
    EISDIR = -21,
    EINVAL = -22,
    ENFILE = -23,
    EMFILE = -24,
    ENOTTY = -25,
    ETXTBSY = -26,
    EFBIG = -27,
    ENOSPC = -28,
    ESPIPE = -29,
    EROFS = -30,
    EMLINK = -31,
    EPIPE = -32,
    EDOM = -33,
    ERANGE = -34,
    EDEADLK = -35,
    ENAMETOOLONG = -36,
    ENOLCK = -37,
    ENOSYS = -38,
    ENOTEMPTY = -39,
    ELOOP = -40,
    ENOMSG = -42,
    EIDRM = -43,
    ECHRNG = -44,
    EL2NSYNC = -45,
    EL3HLT = -46,
    EL3RST = -47,
    ELNRNG = -48,
    EUNATCH = -49,
    ENOCSI = -50,
    EL2HLT = -51,
    EBADE = -52,
    EBADR = -53,
    EXFULL = -54,
    ENOANO = -55,
    EBADRQC = -56,
    EBADSLT = -57,
    EBFONT = -59,
    ENOSTR = -60,
    ENODATA = -61,
    ETIME = -62,
    ENOSR = -63,
    ENONET = -64,
    ENOPKG = -65,
    EREMOTE = -66,
    ENOLINK = -67,
    EADV = -68,
    ESRMNT = -69,
    ECOMM = -70,
    EPROTO = -71,
    EMULTIHOP = -72,
    EDOTDOT = -73,
    EBADMSG = -74,
    EOVERFLOW = -75,
    ENOTUNIQ = -76,
    EBADFD = -77,
    EREMCHG = -78,
    ELIBACC = -79,
    ELIBBAD = -80,
    ELIBSCN = -81,
    ELIBMAX = -82,
    ELIBEXEC = -83,
    EILSEQ = -84,
    ERESTART = -85,
    ESTRPIPE = -86,
    EUSERS = -87,
    ENOTSOCK = -88,
    EDESTADDRREQ = -89,
    EMSGSIZE = -90,
    EPROTOTYPE = -91,
    ENOPROTOOPT = -92,
    EPROTONOSUPPORT = -93,
    ESOCKTNOSUPPORT = -94,
    EOPNOTSUPP = -95,
    EPFNOSUPPORT = -96,
    EAFNOSUPPORT = -97,
    EADDRINUSE = -98,
    EADDRNOTAVAIL = -99,
    ENETDOWN = -100,
    ENETUNREACH = -101,
    ENETRESET = -102,
    ECONNABORTED = -103,
    ECONNRESET = -104,
    ENOBUFS = -105,
    EISCONN = -106,
    ENOTCONN = -107,
    ESHUTDOWN = -108,
    ETOOMANYREFS = -109,
    ETIMEDOUT = -110,
    ECONNREFUSED = -111,
    EHOSTDOWN = -112,
    EHOSTUNREACH = -113,
    EALREADY = -114,
    EINPROGRESS = -115,
    ESTALE = -116,
    EUCLEAN = -117,
    ENOTNAM = -118,
    ENAVAIL = -119,
    EISNAM = -120,
    EREMOTEIO = -121,
    EDQUOT = -122,
    ENOMEDIUM = -123,
    EMEDIUMTYPE = -124,
    ECANCELED = -125,
    ENOKEY = -126,
    EKEYEXPIRED = -127,
    EKEYREVOKED = -128,
    EKEYREJECTED = -129,
    EOWNERDEAD = -130,
    ENOTRECOVERABLE = -131,
    ERFKILL = -132,
    EHWPOISON = -133,
}

impl From<LinuxKernelError> for u64 {
    fn from(e: LinuxKernelError) -> Self {
        e as u64
    }
}

impl LinuxKernelError {
    pub const EWOULDBLOCK: Self = Self::EAGAIN;
    // Resource temporarily unavailable
    pub const ENOTSUP: Self = Self::EOPNOTSUPP;
    // Operation not supported
    pub const EDEADLOCK: Self = Self::EDEADLK; // Resource deadlock avoided

    fn describe(&self) -> &str {
        match self {
            LinuxKernelError::EPERM => "Operation not permitted",
            LinuxKernelError::ENOENT => "No such file or directory",
            LinuxKernelError::ESRCH => "No such process",
            LinuxKernelError::EINTR => "Interrupted system call",
            LinuxKernelError::EIO => "Input/output error",
            LinuxKernelError::ENXIO => "No such device or address",
            LinuxKernelError::E2BIG => "Argument list too long",
            LinuxKernelError::ENOEXEC => "Exec format error",
            LinuxKernelError::EBADF => "Bad file descriptor",
            LinuxKernelError::ECHILD => "No child processes",
            LinuxKernelError::EAGAIN => "Resource temporarily unavailable",
            LinuxKernelError::ENOMEM => "Cannot allocate memory",
            LinuxKernelError::EACCES => "Permission denied",
            LinuxKernelError::EFAULT => "Bad address",
            LinuxKernelError::ENOTBLK => "Block device required",
            LinuxKernelError::EBUSY => "Device or resource busy",
            LinuxKernelError::EEXIST => "File exists",
            LinuxKernelError::EXDEV => "Invalid cross-device link",
            LinuxKernelError::ENODEV => "No such device",
            LinuxKernelError::ENOTDIR => "Not a directory",
            LinuxKernelError::EISDIR => "Is a directory",
            LinuxKernelError::EINVAL => "Invalid argument",
            LinuxKernelError::ENFILE => "Too many open files in system",
            LinuxKernelError::EMFILE => "Too many open files",
            LinuxKernelError::ENOTTY => "Inappropriate ioctl for device",
            LinuxKernelError::ETXTBSY => "Text file busy",
            LinuxKernelError::EFBIG => "File too large",
            LinuxKernelError::ENOSPC => "No space left on device",
            LinuxKernelError::ESPIPE => "Illegal seek",
            LinuxKernelError::EROFS => "Read-only file system",
            LinuxKernelError::EMLINK => "Too many links",
            LinuxKernelError::EPIPE => "Broken pipe",
            LinuxKernelError::EDOM => "Numerical argument out of domain",
            LinuxKernelError::ERANGE => "Numerical result out of range",
            LinuxKernelError::EDEADLK => "Resource deadlock avoided",
            LinuxKernelError::ENAMETOOLONG => "File name too long",
            LinuxKernelError::ENOLCK => "No locks available",
            LinuxKernelError::ENOSYS => "Function not implemented",
            LinuxKernelError::ENOTEMPTY => "Directory not empty",
            LinuxKernelError::ELOOP => "Too many levels of symbolic links",
            LinuxKernelError::ENOMSG => "No message of desired type",
            LinuxKernelError::EIDRM => "Identifier removed",
            LinuxKernelError::ECHRNG => "Channel number out of range",
            LinuxKernelError::EL2NSYNC => "Level 2 not synchronized",
            LinuxKernelError::EL3HLT => "Level 3 halted",
            LinuxKernelError::EL3RST => "Level 3 reset",
            LinuxKernelError::ELNRNG => "Link number out of range",
            LinuxKernelError::EUNATCH => "Protocol driver not attached",
            LinuxKernelError::ENOCSI => "No CSI structure available",
            LinuxKernelError::EL2HLT => "Level 2 halted",
            LinuxKernelError::EBADE => "Invalid exchange",
            LinuxKernelError::EBADR => "Invalid request descriptor",
            LinuxKernelError::EXFULL => "Exchange full",
            LinuxKernelError::ENOANO => "No anode",
            LinuxKernelError::EBADRQC => "Invalid request code",
            LinuxKernelError::EBADSLT => "Invalid slot",
            LinuxKernelError::EBFONT => "Bad font file format",
            LinuxKernelError::ENOSTR => "Device not a stream",
            LinuxKernelError::ENODATA => "No data available",
            LinuxKernelError::ETIME => "Timer expired",
            LinuxKernelError::ENOSR => "Out of streams resources",
            LinuxKernelError::ENONET => "Machine is not on the network",
            LinuxKernelError::ENOPKG => "Package not installed",
            LinuxKernelError::EREMOTE => "Object is remote",
            LinuxKernelError::ENOLINK => "Link has been severed",
            LinuxKernelError::EADV => "Advertise error",
            LinuxKernelError::ESRMNT => "Srmount error",
            LinuxKernelError::ECOMM => "Communication error on send",
            LinuxKernelError::EPROTO => "Protocol error",
            LinuxKernelError::EMULTIHOP => "Multihop attempted",
            LinuxKernelError::EDOTDOT => "RFS specific error",
            LinuxKernelError::EBADMSG => "Bad message",
            LinuxKernelError::EOVERFLOW => "Value too large for defined data type",
            LinuxKernelError::ENOTUNIQ => "Name not unique on network",
            LinuxKernelError::EBADFD => "File descriptor in bad state",
            LinuxKernelError::EREMCHG => "Remote address changed",
            LinuxKernelError::ELIBACC => "Can not access a needed shared library",
            LinuxKernelError::ELIBBAD => "Accessing a corrupted shared library",
            LinuxKernelError::ELIBSCN => ".lib section in a.out corrupted",
            LinuxKernelError::ELIBMAX => "Attempting to link in too many shared libraries",
            LinuxKernelError::ELIBEXEC => "Cannot exec a shared library directly",
            LinuxKernelError::EILSEQ => "Invalid or incomplete multibyte or wide character",
            LinuxKernelError::ERESTART => "Interrupted system call should be restarted",
            LinuxKernelError::ESTRPIPE => "Streams pipe error",
            LinuxKernelError::EUSERS => "Too many users",
            LinuxKernelError::ENOTSOCK => "Socket operation on non-socket",
            LinuxKernelError::EDESTADDRREQ => "Destination address required",
            LinuxKernelError::EMSGSIZE => "Message too long",
            LinuxKernelError::EPROTOTYPE => "Protocol wrong type for socket",
            LinuxKernelError::ENOPROTOOPT => "Protocol not available",
            LinuxKernelError::EPROTONOSUPPORT => "Protocol not supported",
            LinuxKernelError::ESOCKTNOSUPPORT => "Socket type not supported",
            LinuxKernelError::EOPNOTSUPP => "Operation not supported",
            LinuxKernelError::EPFNOSUPPORT => "Protocol family not supported",
            LinuxKernelError::EAFNOSUPPORT => "Address family not supported by protocol",
            LinuxKernelError::EADDRINUSE => "Address already in use",
            LinuxKernelError::EADDRNOTAVAIL => "Cannot assign requested address",
            LinuxKernelError::ENETDOWN => "Network is down",
            LinuxKernelError::ENETUNREACH => "Network is unreachable",
            LinuxKernelError::ENETRESET => "Network dropped connection on reset",
            LinuxKernelError::ECONNABORTED => "Software caused connection abort",
            LinuxKernelError::ECONNRESET => "Connection reset by peer",
            LinuxKernelError::ENOBUFS => "No buffer space available",
            LinuxKernelError::EISCONN => "Transport endpoint is already connected",
            LinuxKernelError::ENOTCONN => "Transport endpoint is not connected",
            LinuxKernelError::ESHUTDOWN => "Cannot send after transport endpoint shutdown",
            LinuxKernelError::ETOOMANYREFS => "Too many references: cannot splice",
            LinuxKernelError::ETIMEDOUT => "Connection timed out",
            LinuxKernelError::ECONNREFUSED => "Connection refused",
            LinuxKernelError::EHOSTDOWN => "Host is down",
            LinuxKernelError::EHOSTUNREACH => "No route to host",
            LinuxKernelError::EALREADY => "Operation already in progress",
            LinuxKernelError::EINPROGRESS => "Operation now in progress",
            LinuxKernelError::ESTALE => "Stale file handle",
            LinuxKernelError::EUCLEAN => "Structure needs cleaning",
            LinuxKernelError::ENOTNAM => "Not a XENIX named type file",
            LinuxKernelError::ENAVAIL => "No XENIX semaphores available",
            LinuxKernelError::EISNAM => "Is a named type file",
            LinuxKernelError::EREMOTEIO => "Remote I/O error",
            LinuxKernelError::EDQUOT => "Disk quota exceeded",
            LinuxKernelError::ENOMEDIUM => "No medium found",
            LinuxKernelError::EMEDIUMTYPE => "Wrong medium type",
            LinuxKernelError::ECANCELED => "Operation canceled",
            LinuxKernelError::ENOKEY => "Required key not available",
            LinuxKernelError::EKEYEXPIRED => "Key has expired",
            LinuxKernelError::EKEYREVOKED => "Key has been revoked",
            LinuxKernelError::EKEYREJECTED => "Key was rejected by service",
            LinuxKernelError::EOWNERDEAD => "Owner died",
            LinuxKernelError::ENOTRECOVERABLE => "State not recoverable",
            LinuxKernelError::ERFKILL => "Operation not possible due to RF-kill",
            LinuxKernelError::EHWPOISON => "Memory page has hardware error",
        }
    }
}

impl From<CPUError> for LinuxKernelError {
    fn from(_: CPUError) -> Self {
        LinuxKernelError::EINVAL
    }
}