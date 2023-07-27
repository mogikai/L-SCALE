use std::backtrace::Backtrace;
use std::convert::Infallible;
use std::error::Error;

use thiserror::Error;

use crate::emulator::abstractions::cpu::CPUError;
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::file_descriptors::FileDescriptorError;
use crate::emulator::abstractions::mmu::MemoryError;
use crate::emulator::system::fd_map::error::FileDescriptorMapError;
use crate::emulator::system::filesystem::FSError;

#[derive(Error, Debug, Copy, Clone)]
#[repr(i64)]
pub enum LinuxKernelError {
    #[error("EPERM: Operation not permitted")]
    EPERM           = -1,
    #[error("ENOENT: No such file or directory")]
    ENOENT          = -2,
    #[error("ESRCH: No such process")]
    ESRCH           = -3,
    #[error("EINTR: Interrupted system call")]
    EINTR           = -4,
    #[error("EIO: Input/output error")]
    EIO             = -5,
    #[error("ENXIO: No such device or address")]
    ENXIO           = -6,
    #[error("E2BIG: Argument list too long")]
    E2BIG           = -7,
    #[error("ENOEXEC: Exec format error")]
    ENOEXEC         = -8,
    #[error("EBADF: Bad file descriptor")]
    EBADF           = -9,
    #[error("ECHILD: No child processes")]
    ECHILD          = -10,
    #[error("EAGAIN: Resource temporarily unavailable")]
    EAGAIN          = -11,
    #[error("ENOMEM: Cannot allocate memory")]
    ENOMEM          = -12,
    #[error("EACCES: Permission denied")]
    EACCES          = -13,
    #[error("EFAULT: Bad address")]
    EFAULT          = -14,
    #[error("ENOTBLK: Block device required")]
    ENOTBLK         = -15,
    #[error("EBUSY: Device or resource busy")]
    EBUSY           = -16,
    #[error("EEXIST: File exists")]
    EEXIST          = -17,
    #[error("EXDEV: Invalid cross-device link")]
    EXDEV           = -18,
    #[error("ENODEV: No such device")]
    ENODEV          = -19,
    #[error("ENOTDIR: Not a directory")]
    ENOTDIR         = -20,
    #[error("EISDIR: Is a directory")]
    EISDIR          = -21,
    #[error("EINVAL: Invalid argument")]
    EINVAL          = -22,
    #[error("ENFILE: Too many open files in system")]
    ENFILE          = -23,
    #[error("EMFILE: Too many open files")]
    EMFILE          = -24,
    #[error("ENOTTY: Inappropriate ioctl for device")]
    ENOTTY          = -25,
    #[error("ETXTBSY: Text file busy")]
    ETXTBSY         = -26,
    #[error("EFBIG: File too large")]
    EFBIG           = -27,
    #[error("ENOSPC: No space left on device")]
    ENOSPC          = -28,
    #[error("ESPIPE: Illegal seek")]
    ESPIPE          = -29,
    #[error("EROFS: Read-only file system")]
    EROFS           = -30,
    #[error("EMLINK: Too many links")]
    EMLINK          = -31,
    #[error("EPIPE: Broken pipe")]
    EPIPE           = -32,
    #[error("EDOM: Numerical argument out of domain")]
    EDOM            = -33,
    #[error("ERANGE: Numerical result out of range")]
    ERANGE          = -34,
    #[error("EDEADLK: Resource deadlock avoided")]
    EDEADLK         = -35,
    #[error("ENAMETOOLONG: File name too long")]
    ENAMETOOLONG    = -36,
    #[error("ENOLCK: No locks available")]
    ENOLCK          = -37,
    #[error("ENOSYS: Function not implemented")]
    ENOSYS          = -38,
    #[error("ENOTEMPTY: Directory not empty")]
    ENOTEMPTY       = -39,
    #[error("ELOOP: Too many levels of symbolic links")]
    ELOOP           = -40,
    #[error("ENOMSG: No message of desired type")]
    ENOMSG          = -42,
    #[error("EIDRM: Identifier removed")]
    EIDRM           = -43,
    #[error("ECHRNG: Channel number out of range")]
    ECHRNG          = -44,
    #[error("EL2NSYNC: Level 2 not synchronized")]
    EL2NSYNC        = -45,
    #[error("EL3HLT: Level 3 halted")]
    EL3HLT          = -46,
    #[error("EL3RST: Level 3 reset")]
    EL3RST          = -47,
    #[error("ELNRNG: Link number out of range")]
    ELNRNG          = -48,
    #[error("EUNATCH: Protocol driver not attached")]
    EUNATCH         = -49,
    #[error("ENOCSI: No CSI structure available")]
    ENOCSI          = -50,
    #[error("EL2HLT: Level 2 halted")]
    EL2HLT          = -51,
    #[error("EBADE: Invalid exchange")]
    EBADE           = -52,
    #[error("EBADR: Invalid request descriptor")]
    EBADR           = -53,
    #[error("EXFULL: Exchange full")]
    EXFULL          = -54,
    #[error("ENOANO: No anode")]
    ENOANO          = -55,
    #[error("EBADRQC: Invalid request code")]
    EBADRQC         = -56,
    #[error("EBADSLT: Invalid slot")]
    EBADSLT         = -57,
    #[error("EBFONT: Bad font file format")]
    EBFONT          = -59,
    #[error("ENOSTR: Device not a stream")]
    ENOSTR          = -60,
    #[error("ENODATA: No data available")]
    ENODATA         = -61,
    #[error("ETIME: Timer expired")]
    ETIME           = -62,
    #[error("ENOSR: Out of streams resources")]
    ENOSR           = -63,
    #[error("ENONET: Machine is not on the network")]
    ENONET          = -64,
    #[error("ENOPKG: Package not installed")]
    ENOPKG          = -65,
    #[error("EREMOTE: Object is remote")]
    EREMOTE         = -66,
    #[error("ENOLINK: Link has been severed")]
    ENOLINK         = -67,
    #[error("EADV: Advertise error")]
    EADV            = -68,
    #[error("ESRMNT: Srmount error")]
    ESRMNT          = -69,
    #[error("ECOMM: Communication error on send")]
    ECOMM           = -70,
    #[error("EPROTO: Protocol error")]
    EPROTO          = -71,
    #[error("EMULTIHOP: Multihop attempted")]
    EMULTIHOP       = -72,
    #[error("EDOTDOT: RFS specific error")]
    EDOTDOT         = -73,
    #[error("EBADMSG: Bad message")]
    EBADMSG         = -74,
    #[error("EOVERFLOW: Value too large for defined data type")]
    EOVERFLOW       = -75,
    #[error("ENOTUNIQ: Name not unique on network")]
    ENOTUNIQ        = -76,
    #[error("EBADFD: File descriptor in bad state")]
    EBADFD          = -77,
    #[error("EREMCHG: Remote address changed")]
    EREMCHG         = -78,
    #[error("ELIBACC: Can not access a needed shared library")]
    ELIBACC         = -79,
    #[error("ELIBBAD: Accessing a corrupted shared library")]
    ELIBBAD         = -80,
    #[error("ELIBSCN: .lib section in a.out corrupted")]
    ELIBSCN         = -81,
    #[error("ELIBMAX: Attempting to link in too many shared libraries")]
    ELIBMAX         = -82,
    #[error("ELIBEXEC: Cannot exec a shared library directly")]
    ELIBEXEC        = -83,
    #[error("EILSEQ: Invalid or incomplete multibyte or wide character")]
    EILSEQ          = -84,
    #[error("ERESTART: Interrupted system call should be restarted")]
    ERESTART        = -85,
    #[error("ESTRPIPE: Streams pipe error")]
    ESTRPIPE        = -86,
    #[error("EUSERS: Too many users")]
    EUSERS          = -87,
    #[error("ENOTSOCK: Socket operation on non-socket")]
    ENOTSOCK        = -88,
    #[error("EDESTADDRREQ: Destination address required")]
    EDESTADDRREQ    = -89,
    #[error("EMSGSIZE: Message too long")]
    EMSGSIZE        = -90,
    #[error("EPROTOTYPE: Protocol wrong type for socket")]
    EPROTOTYPE      = -91,
    #[error("ENOPROTOOPT: Protocol not available")]
    ENOPROTOOPT     = -92,
    #[error("EPROTONOSUPPORT: Protocol not supported")]
    EPROTONOSUPPORT = -93,
    #[error("ESOCKTNOSUPPORT: Socket type not supported")]
    ESOCKTNOSUPPORT = -94,
    #[error("EOPNOTSUPP: Operation not supported")]
    EOPNOTSUPP      = -95,
    #[error("EPFNOSUPPORT: Protocol family not supported")]
    EPFNOSUPPORT    = -96,
    #[error("EAFNOSUPPORT: Address family not supported by protocol")]
    EAFNOSUPPORT    = -97,
    #[error("EADDRINUSE: Address already in use")]
    EADDRINUSE      = -98,
    #[error("EADDRNOTAVAIL: Cannot assign requested address")]
    EADDRNOTAVAIL   = -99,
    #[error("ENETDOWN: Network is down")]
    ENETDOWN        = -100,
    #[error("ENETUNREACH: Network is unreachable")]
    ENETUNREACH     = -101,
    #[error("ENETRESET: Network dropped connection on reset")]
    ENETRESET       = -102,
    #[error("ECONNABORTED: Software caused connection abort")]
    ECONNABORTED    = -103,
    #[error("ECONNRESET: Connection reset by peer")]
    ECONNRESET      = -104,
    #[error("ENOBUFS: No buffer space available")]
    ENOBUFS         = -105,
    #[error("EISCONN: Transport endpoint is already connected")]
    EISCONN         = -106,
    #[error("ENOTCONN: Transport endpoint is not connected")]
    ENOTCONN        = -107,
    #[error("ESHUTDOWN: Cannot send after transport endpoint shutdown")]
    ESHUTDOWN       = -108,
    #[error("ETOOMANYREFS: Too many references: cannot splice")]
    ETOOMANYREFS    = -109,
    #[error("ETIMEDOUT: Connection timed out")]
    ETIMEDOUT       = -110,
    #[error("ECONNREFUSED: Connection refused")]
    ECONNREFUSED    = -111,
    #[error("EHOSTDOWN: Host is down")]
    EHOSTDOWN       = -112,
    #[error("EHOSTUNREACH: No route to host")]
    EHOSTUNREACH    = -113,
    #[error("EALREADY: Operation already in progress")]
    EALREADY        = -114,
    #[error("EINPROGRESS: Operation now in progress")]
    EINPROGRESS     = -115,
    #[error("ESTALE: Stale file handle")]
    ESTALE          = -116,
    #[error("EUCLEAN: Structure needs cleaning")]
    EUCLEAN         = -117,
    #[error("ENOTNAM: Not a XENIX named type file")]
    ENOTNAM         = -118,
    #[error("ENAVAIL: No XENIX semaphores available")]
    ENAVAIL         = -119,
    #[error("EISNAM: Is a named type file")]
    EISNAM          = -120,
    #[error("EREMOTEIO: Remote I/O error")]
    EREMOTEIO       = -121,
    #[error("EDQUOT: Disk quota exceeded")]
    EDQUOT          = -122,
    #[error("ENOMEDIUM: No medium found")]
    ENOMEDIUM       = -123,
    #[error("EMEDIUMTYPE: Wrong medium type")]
    EMEDIUMTYPE     = -124,
    #[error("ECANCELED: Operation canceled")]
    ECANCELED       = -125,
    #[error("ENOKEY: Required key not available")]
    ENOKEY          = -126,
    #[error("EKEYEXPIRED: Key has expired")]
    EKEYEXPIRED     = -127,
    #[error("EKEYREVOKED: Key has been revoked")]
    EKEYREVOKED     = -128,
    #[error("EKEYREJECTED: Key was rejected by service")]
    EKEYREJECTED    = -129,
    #[error("EOWNERDEAD: Owner died")]
    EOWNERDEAD      = -130,
    #[error("ENOTRECOVERABLE: State not recoverable")]
    ENOTRECOVERABLE = -131,
    #[error("ERFKILL: Operation not possible due to RF-kill")]
    ERFKILL         = -132,
    #[error("EHWPOISON: Memory page has hardware error")]
    EHWPOISON       = -133,
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
}

impl From<FileDescriptorMapError> for LinuxKernelError {
    fn from(e: FileDescriptorMapError) -> Self {
        match e {
            FileDescriptorMapError::InvalidFD => LinuxKernelError::EBADF,
            FileDescriptorMapError::NoFreeFD => todo!(),
            FileDescriptorMapError::FDExists => todo!(),
        }
    }
}

impl From<CPUError> for LinuxKernelError {
    fn from(_: CPUError) -> Self {
        LinuxKernelError::EINVAL
    }
}

impl From<FSError> for LinuxKernelError {
    fn from(e: FSError) -> Self {
        match e {
            FSError::NotFound => LinuxKernelError::ENOENT,
            _ => LinuxKernelError::EINVAL,
        }
    }
}

impl From<MemoryError> for LinuxKernelError {
    fn from(me: MemoryError) -> Self {
        use MemoryError::*;
        match me {
            NotMapped => LinuxKernelError::EFAULT,
            NotContinuous | OutOfMemory => LinuxKernelError::ENOMEM,
            _ => LinuxKernelError::EINVAL,
        }
    }
}

impl From<GenericError> for LinuxKernelError {
    fn from(e: GenericError) -> Self {
        match e.kind() {
            GenericErrorKind::CPUError(e) => (*e).into(),
            GenericErrorKind::FSError(e) => (*e).into(),
            GenericErrorKind::MemoryError(e) => (*e).into(),
            GenericErrorKind::Unknown => LinuxKernelError::EINVAL,
            GenericErrorKind::NotImplemented => LinuxKernelError::EINVAL,
        }
    }
}

impl From<FileDescriptorError> for LinuxKernelError {
    fn from(value: FileDescriptorError) -> Self {
        match value {
            FileDescriptorError::FileNotFound => LinuxKernelError::ENOENT,
            FileDescriptorError::NotSupported => LinuxKernelError::EOPNOTSUPP,
            FileDescriptorError::NotASocket => LinuxKernelError::ENOTSOCK,
            FileDescriptorError::OperationFailed => LinuxKernelError::EINVAL,
            FileDescriptorError::Unknown => LinuxKernelError::EINVAL,
            FileDescriptorError::MemoryError(e) => e.into(),
            FileDescriptorError::FSError(e) => e.into(),
        }
    }
}

#[derive(Error, Debug)]
pub enum SyscallError {
    #[error("Linux Kernel error occurred")]
    KernelError(LinuxKernelError),
    #[error("The kernel is yielding to the scheduler")]
    Yield,
    #[error("The requested operation is not yet implemented")]
    NotImplemented,
}

impl<T: Into<LinuxKernelError>> From<T> for SyscallError {
    fn from(value: T) -> Self {
        SyscallError::KernelError(value.into())
    }
}

impl From<Infallible> for SyscallError {
    fn from(value: Infallible) -> Self {
        unimplemented!()
    }
}
