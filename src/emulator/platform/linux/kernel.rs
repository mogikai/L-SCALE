use std::collections::HashMap;
use std::path::Path;

use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::{CPU, CPUError, RegisterX86, Vaddr};
use crate::emulator::abstractions::handles::{Handle, IOErr};
use crate::emulator::platform::linux::filedescriptors::OpenFile;
use crate::emulator::system::default_handles;
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory;
use crate::emulator::system::memory::{Memory, MemoryWrite};
use crate::read_osstring;

const MAP_ANONYMOUS: u64 = 0x20;
const MAP_PRIVATE: u64 = 0x2;
const MAP_FIXED: u64 = 0x10;
const MAP_DENYWRITE: u64 = 0x0800;

#[derive(Debug)]
pub enum LinuxKernelError {
    EPERM = 1,
    // Operation not permitted
    ENOENT = 2,
    // No such file or directory
    ESRCH = 3,
    // No such process
    EINTR = 4,
    // Interrupted system call
    EIO = 5,
    // Input/output error
    ENXIO = 6,
    // No such device or address
    E2BIG = 7,
    // Argument list too long
    ENOEXEC = 8,
    // Exec format error
    EBADF = 9,
    // Bad file descriptor
    ECHILD = 10,
    // No child processes
    EAGAIN = 11,
    // Resource temporarily unavailable
    ENOMEM = 12,
    // Cannot allocate memory
    EACCES = 13,
    // Permission denied
    EFAULT = 14,
    // Bad address
    ENOTBLK = 15,
    // Block device required
    EBUSY = 16,
    // Device or resource busy
    EEXIST = 17,
    // File exists
    EXDEV = 18,
    // Invalid cross-device link
    ENODEV = 19,
    // No such device
    ENOTDIR = 20,
    // Not a directory
    EISDIR = 21,
    // Is a directory
    EINVAL = 22,
    // Invalid argument
    ENFILE = 23,
    // Too many open files in system
    EMFILE = 24,
    // Too many open files
    ENOTTY = 25,
    // Inappropriate ioctl for device
    ETXTBSY = 26,
    // Text file busy
    EFBIG = 27,
    // File too large
    ENOSPC = 28,
    // No space left on device
    ESPIPE = 29,
    // Illegal seek
    EROFS = 30,
    // Read-only file system
    EMLINK = 31,
    // Too many links
    EPIPE = 32,
    // Broken pipe
    EDOM = 33,
    // Numerical argument out of domain
    ERANGE = 34,
    // Numerical result out of range
    EDEADLK = 35,
    // Resource deadlock avoided
    ENAMETOOLONG = 36,
    // File name too long
    ENOLCK = 37,
    // No locks available
    ENOSYS = 38,
    // Function not implemented
    ENOTEMPTY = 39,
    // Directory not empty
    ELOOP = 40,
    // Too many levels of symbolic links
    ENOMSG = 42,
    // No message of desired type
    EIDRM = 43,
    // Identifier removed
    ECHRNG = 44,
    // Channel number out of range
    EL2NSYNC = 45,
    // Level 2 not synchronized
    EL3HLT = 46,
    // Level 3 halted
    EL3RST = 47,
    // Level 3 reset
    ELNRNG = 48,
    // Link number out of range
    EUNATCH = 49,
    // Protocol driver not attached
    ENOCSI = 50,
    // No CSI structure available
    EL2HLT = 51,
    // Level 2 halted
    EBADE = 52,
    // Invalid exchange
    EBADR = 53,
    // Invalid request descriptor
    EXFULL = 54,
    // Exchange full
    ENOANO = 55,
    // No anode
    EBADRQC = 56,
    // Invalid request code
    EBADSLT = 57,
    // Invalid slot
    EBFONT = 59,
    // Bad font file format
    ENOSTR = 60,
    // Device not a stream
    ENODATA = 61,
    // No data available
    ETIME = 62,
    // Timer expired
    ENOSR = 63,
    // Out of streams resources
    ENONET = 64,
    // Machine is not on the network
    ENOPKG = 65,
    // Package not installed
    EREMOTE = 66,
    // Object is remote
    ENOLINK = 67,
    // Link has been severed
    EADV = 68,
    // Advertise error
    ESRMNT = 69,
    // Srmount error
    ECOMM = 70,
    // Communication error on send
    EPROTO = 71,
    // Protocol error
    EMULTIHOP = 72,
    // Multihop attempted
    EDOTDOT = 73,
    // RFS specific error
    EBADMSG = 74,
    // Bad message
    EOVERFLOW = 75,
    // Value too large for defined data type
    ENOTUNIQ = 76,
    // Name not unique on network
    EBADFD = 77,
    // File descriptor in bad state
    EREMCHG = 78,
    // Remote address changed
    ELIBACC = 79,
    // Can not access a needed shared library
    ELIBBAD = 80,
    // Accessing a corrupted shared library
    ELIBSCN = 81,
    // .lib section in a.out corrupted
    ELIBMAX = 82,
    // Attempting to link in too many shared libraries
    ELIBEXEC = 83,
    // Cannot exec a shared library directly
    EILSEQ = 84,
    // Invalid or incomplete multibyte or wide character
    ERESTART = 85,
    // Interrupted system call should be restarted
    ESTRPIPE = 86,
    // Streams pipe error
    EUSERS = 87,
    // Too many users
    ENOTSOCK = 88,
    // Socket operation on non-socket
    EDESTADDRREQ = 89,
    // Destination address required
    EMSGSIZE = 90,
    // Message too long
    EPROTOTYPE = 91,
    // Protocol wrong type for socket
    ENOPROTOOPT = 92,
    // Protocol not available
    EPROTONOSUPPORT = 93,
    // Protocol not supported
    ESOCKTNOSUPPORT = 94,
    // Socket type not supported
    EOPNOTSUPP = 95,
    // Operation not supported
    EPFNOSUPPORT = 96,
    // Protocol family not supported
    EAFNOSUPPORT = 97,
    // Address family not supported by protocol
    EADDRINUSE = 98,
    // Address already in use
    EADDRNOTAVAIL = 99,
    // Cannot assign requested address
    ENETDOWN = 100,
    // Network is down
    ENETUNREACH = 101,
    // Network is unreachable
    ENETRESET = 102,
    // Network dropped connection on reset
    ECONNABORTED = 103,
    // Software caused connection abort
    ECONNRESET = 104,
    // Connection reset by peer
    ENOBUFS = 105,
    // No buffer space available
    EISCONN = 106,
    // Transport endpoint is already connected
    ENOTCONN = 107,
    // Transport endpoint is not connected
    ESHUTDOWN = 108,
    // Cannot send after transport endpoint shutdown
    ETOOMANYREFS = 109,
    // Too many references: cannot splice
    ETIMEDOUT = 110,
    // Connection timed out
    ECONNREFUSED = 111,
    // Connection refused
    EHOSTDOWN = 112,
    // Host is down
    EHOSTUNREACH = 113,
    // No route to host
    EALREADY = 114,
    // Operation already in progress
    EINPROGRESS = 115,
    // Operation now in progress
    ESTALE = 116,
    // Stale file handle
    EUCLEAN = 117,
    // Structure needs cleaning
    ENOTNAM = 118,
    // Not a XENIX named type file
    ENAVAIL = 119,
    // No XENIX semaphores available
    EISNAM = 120,
    // Is a named type file
    EREMOTEIO = 121,
    // Remote I/O error
    EDQUOT = 122,
    // Disk quota exceeded
    ENOMEDIUM = 123,
    // No medium found
    EMEDIUMTYPE = 124,
    // Wrong medium type
    ECANCELED = 125,
    // Operation canceled
    ENOKEY = 126,
    // Required key not available
    EKEYEXPIRED = 127,
    // Key has expired
    EKEYREVOKED = 128,
    // Key has been revoked
    EKEYREJECTED = 129,
    // Key was rejected by service
    EOWNERDEAD = 130,
    // Owner died
    ENOTRECOVERABLE = 131,
    // State not recoverable
    ERFKILL = 132,
    // Operation not possible due to RF-kill
    EHWPOISON = 133, // Memory page has hardware error
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

pub struct LinuxKernel<Endianness: ByteOrder> {
    memory: Memory<Endianness>,
    filesystem: Filesystem,
    handles: HashMap<u64, Box<dyn Handle>>,
}

impl<Endianness: ByteOrder> LinuxKernel<Endianness> {
    pub fn new(memory: Memory<Endianness>, filesystem: Filesystem) -> LinuxKernel<Endianness> {
        let mut handles: HashMap<u64, Box<dyn Handle>> = HashMap::new();
        handles.insert(0, Box::new(default_handles::WriteOnly {}));
        handles.insert(1, Box::new(default_handles::WriteOnly {}));
        handles.insert(2, Box::new(default_handles::WriteOnly {}));
        LinuxKernel { memory, filesystem, handles }
    }

    pub fn memory(&self) -> &Memory<Endianness> { &self.memory }
    pub fn memory_mut(&mut self) -> &mut Memory<Endianness> { &mut self.memory }
    pub fn filesystem(&self) -> &Filesystem { &self.filesystem }
    pub fn filesystem_mut(&mut self) -> &mut Filesystem { &mut self.filesystem }

    pub fn brk<'a, CpuT: CPU<'a>>(&mut self, cpu: &mut CpuT, addr: u64) -> u64 {
        if addr != 0 {
            self.memory.realloc_heap(addr, cpu);
        }
        self.memory.heapend()
    }
    pub fn arch_prctl<'a, CpuT: CPU<'a>>(&mut self, cpu: &mut CpuT, code: u64, addr: u64) -> u64 {
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
    pub fn write<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, fd: u64, buf: u64, size: u64) -> u64 {
        self.handles.get_mut(&fd).map_or(!0, |handle| {
            let data = self.memory.read_internal(buf, size as usize).unwrap();
            handle.write(&*data).unwrap_or(!0).try_into().unwrap()
        })
    }

    pub fn writev<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, fd: u64, iovec_ptr: Vaddr, iovcnt: u64) -> u64 {
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
    pub fn openat<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, _dfd: u64, filename: u64, _flags: u64, _mode: u64) -> u64 {
        let fname = read_osstring!(self.memory, filename).unwrap();
        let p = Path::new(&fname);
        let file = self.filesystem.get(p);
        if let Some(file) = file {
            let fd: u64 = *self.handles.keys().max().unwrap_or(&2) + 1;
            self.handles.insert(fd, Box::new(OpenFile::new(file)));
            return fd;
        }
        !0
    }
    pub fn read<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, fd: u64, buf: u64, count: u64) -> u64 {
        if let Some(f) = self.handles.get_mut(&fd) {
            self.memory.writeat(buf, f.read(count).unwrap())
        } else {
            !0
        }
    }
    pub fn getpid<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT) -> u64 {
        // TODO: Implement multi threading / processing
        1
    }
    fn mmap_anon<'a, CpuT: CPU<'a>>(&mut self, cpu: &mut CpuT, addr: Vaddr, length: u64, prot: u64, flags: u64) -> Result<u64, LinuxKernelError> {
        let addr = if (flags & MAP_FIXED) != 0 {
            if let Err(e) = self.memory.unmap(addr, length, cpu) {
                return Ok(match e {
                    memory::UnmapError::NotAligned => !0,
                    memory::UnmapError::ZeroSize => !0,
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
                return Ok(!0);
            }
        };
        Ok(addr)
    }
    fn mmap_private<'a, CpuT: CPU<'a>>(&mut self, cpu: &mut CpuT, addr: u64, length: u64, prot: u64, flags: u64, fd: u64, offset: u64) -> u64 {
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
        self.memory.writeat(addr, data);
        addr
    }
    pub fn mmap<'a, CpuT: CPU<'a>>(&mut self, cpu: &mut CpuT, addr: Vaddr, length: u64, prot: u64, flags: u64, fd: u64, offset: u64) -> u64 {
        if (flags & !(MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED | MAP_DENYWRITE)) != 0 {
            return !0;
        }
        if (flags & MAP_ANONYMOUS) != 0 {
            return self.mmap_anon(cpu, addr, length, prot, flags).unwrap();
        }
        if (flags & MAP_PRIVATE) != 0 {
            return self.mmap_private(cpu, addr, length, prot, flags, fd, offset);
        }
        !0
    }
    pub fn close<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, fd: u64) -> u64 {
        self.handles.remove(&fd);
        0
    }
    pub fn uname<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, buf: u64) -> u64 {
        //TODO check if address is mapped and has space for struct new_utsname
        self.memory.writeat(buf + 0 * 65, "Linux");
        self.memory.writeat(buf + 1 * 65, "ILT792");
        self.memory.writeat(buf + 2 * 65, "5.4.0-110-generic");
        self.memory.writeat(buf + 3 * 65, "#124-Ubuntu SMP Thu Apr 14 19:46:19 UTC 2022");
        self.memory.writeat(buf + 4 * 65, "x86_64");
        self.memory.writeat(buf + 5 * 65, "(none)");
        0
    }
    pub fn access<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, _filename: u64, _mode: u64) -> u64 {
        !0
    }
    pub fn fstat<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, fd: u64, statbuf: Vaddr) -> u64 {
        let file = self.handles.get_mut(&fd);
        match file {
            Some(f) => {
                self.memory.writeat(statbuf, &f.stat());
                return 0;
            }
            None => {}
        }
        !0
    }
    pub fn stat<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, filename: u64, statbuf: Vaddr) -> u64 {
        let fname = read_osstring!(self.memory, filename).unwrap();
        match self.filesystem.stat(Path::new(&fname)) {
            Ok(s) => {
                self.memory.writeat(statbuf, &s);
                0
            }
            Err(_) => !0,
        }
    }
    pub fn pread64<'a, CpuT: CPU<'a>>(&mut self, _: &mut CpuT, fd: u64, buf: u64, count: u64, pos: u64) -> u64 {
        let file = self.handles.get_mut(&fd);
        if let Some(f) = file {
            let d = f.pread(pos, count).unwrap();
            self.memory.writeat(buf, d)
        } else {
            !0
        }
    }
    pub fn mprotect<'a, CpuT: CPU<'a>>(&mut self, cpu: &mut CpuT, start: u64, len: u64, prot: u64) -> u64 {
        self.memory.protect(start, len, prot.try_into().unwrap(), cpu);
        0
    }
}