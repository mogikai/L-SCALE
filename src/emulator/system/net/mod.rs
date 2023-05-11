use byteorder::ByteOrder;
use num_enum::FromPrimitive;
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};

use crate::emulator::abstractions::mmu::MemoryError;
use crate::emulator::platform::linux::kernel::LinuxKernelError;

pub mod inet6;
pub mod netlink;
pub mod unix;

#[derive(Copy, Clone)]
pub enum Sockaddr {
    IN {
        sin_port: u16,
        sin_addr: u32,
    },
    NL {
        nl_pad: u16,
        nl_pid: u32,
        nl_groups: u32,
    },
    IN6 {
        sin6_port: u16,      /* Transport layer port # */
        sin6_flowinfo: u32,  /* IPv6 flow information */
        sin6_addr: [u8; 16], /* IPv6 address */
        sin6_scope_id: u32,  /* IPv6 scope-id */
    },
}

impl<Endianness: ByteOrder> Decode<Endianness> for Sockaddr {
    fn decode(val: &[u8]) -> Result<Self, MemoryError> {
        let family = SocketDomain::from(Endianness::read_u16(&val[..2]) as u64);
        use SocketDomain::*;
        Ok(match family {
            AF_INET => Sockaddr::IN {
                sin_port: Endianness::read_u16(&val[2..4]),
                sin_addr: Endianness::read_u32(&val[4..8]),
            },
            AF_NETLINK => Sockaddr::NL {
                nl_pad: Endianness::read_u16(&val[2..4]),
                nl_pid: Endianness::read_u32(&val[4..8]),
                nl_groups: Endianness::read_u32(&val[8..12]),
            },
            AF_INET6 => Sockaddr::IN6 {
                sin6_port: Endianness::read_u16(&val[2..4]),
                sin6_flowinfo: Endianness::read_u32(&val[4..8]),
                sin6_addr: <[u8; 16]>::try_from(&val[8..24]).unwrap(),
                sin6_scope_id: Endianness::read_u32(&val[24..28]),
            },
            _ => {
                error!("Sockaddr not implemented for type {:?}", family);
                Err(MemoryError::ConversionError)?
            }
        })
    }
}

impl RequiredMem for Sockaddr {
    fn required_mem() -> usize {
        128 // Defined in kernel
    }
}

impl<Endianness: ByteOrder> Encode<Endianness> for Sockaddr {
    fn encode(&self) -> Vec<u8> {
        let mut out = vec![0; Self::required_mem()];
        use Sockaddr::*;
        match self {
            NL {
                nl_pad,
                nl_pid,
                nl_groups,
            } => {
                Endianness::write_u16(&mut out[..2], SocketDomain::AF_NETLINK as u16);
                Endianness::write_u16(&mut out[2..4], *nl_pad);
                Endianness::write_u32(&mut out[4..8], *nl_pid);
                Endianness::write_u32(&mut out[8..12], *nl_groups);
            }
            _ => todo!(),
        }
        out
    }
}

pub struct SocketFlags {
    flags: u64,
}

impl SocketFlags {
    fn SOCK_CLOEXEC(&self) -> bool {
        self.flags & 0x02000000 != 0
    }

    fn SOCK_NONBLOCK(&self) -> bool {
        self.flags & 0x00004000 != 0
    }
}

impl From<u64> for SocketFlags {
    fn from(val: u64) -> Self {
        Self { flags: val & !0xFFF }
    }
}

#[derive(Debug)]
pub enum SocketType {
    SOCK_STREAM    = 1,
    SOCK_DGRAM     = 2,
    SOCK_RAW       = 3,
    SOCK_RDM       = 4,
    SOCK_SEQPACKET = 5,
    SOCK_DCCP      = 6,
    SOCK_PACKET    = 10,
}

impl TryFrom<u64> for SocketType {
    type Error = LinuxKernelError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        use SocketType::*;
        Ok(match value & 0xFF {
            1 => SOCK_STREAM,
            2 => SOCK_DGRAM,
            3 => SOCK_RAW,
            4 => SOCK_RDM,
            5 => SOCK_SEQPACKET,
            6 => SOCK_DCCP,
            10 => SOCK_PACKET,
            _ => Err(LinuxKernelError::EINVAL)?,
        })
    }
}

#[repr(u64)]
#[allow(non_camel_case_types)]
#[derive(Debug, FromPrimitive)]
pub enum SocketDomain {
    #[default]
    AF_UNSPEC     = 0,
    AF_UNIX       = 1,
    AF_INET       = 2,
    AF_AX25       = 3,
    AF_IPX        = 4,
    AF_APPLETALK  = 5,
    AF_NETROM     = 6,
    AF_BRIDGE     = 7,
    AF_ATMPVC     = 8,
    AF_X25        = 9,
    AF_INET6      = 10,
    AF_ROSE       = 11,
    AF_DECnet     = 12,
    AF_NETBEUI    = 13,
    AF_SECURITY   = 14,
    AF_KEY        = 15,
    AF_NETLINK    = 16,
    AF_PACKET     = 17,
    AF_ASH        = 18,
    AF_ECONET     = 19,
    AF_ATMSVC     = 20,
    AF_RDS        = 21,
    AF_SNA        = 22,
    AF_IRDA       = 23,
    AF_PPPOX      = 24,
    AF_WANPIPE    = 25,
    AF_LLC        = 26,
    AF_CAN        = 29,
    AF_TIPC       = 30,
    AF_BLUETOOTH  = 31,
    AF_IUCV       = 32,
    AF_RXRPC      = 33,
    AF_ISDN       = 34,
    AF_PHONET     = 35,
    AF_IEEE802154 = 36,
    AF_CAIF       = 37,
    AF_ALG        = 38,
}

pub enum SockOptLevel {
    SOL_SOCKET = 1,
}

pub enum SockOpt {
    SO_DEBUG            = 1,
    SO_REUSEADDR        = 2,
    SO_TYPE             = 3,
    SO_ERROR            = 4,
    SO_DONTROUTE        = 5,
    SO_BROADCAST        = 6,
    SO_SNDBUF           = 7,
    SO_RCVBUF           = 8,
    SO_SNDBUFFORCE      = 32,
    SO_RCVBUFFORCE      = 33,
    SO_KEEPALIVE        = 9,
    SO_OOBINLINE        = 10,
    SO_NO_CHECK         = 11,
    SO_PRIORITY         = 12,
    SO_LINGER           = 13,
    SO_BSDCOMPAT        = 14,
    SO_REUSEPORT        = 15,

    SO_PASSCRED         = 16,
    SO_PEERCRED         = 17,
    SO_RCVLOWAT         = 18,
    SO_SNDLOWAT         = 19,
    SO_RCVTIMEO_OLD     = 20,
    SO_SNDTIMEO_OLD     = 21,

    /* Security levels - as per NRL IPv6 - don't actually do anything */
    SO_SECURITY_AUTHENTICATION = 22,
    SO_SECURITY_ENCRYPTION_TRANSPORT = 23,
    SO_SECURITY_ENCRYPTION_NETWORK = 24,

    SO_BINDTODEVICE     = 25,

    /* Socket filtering */
    SO_ATTACH_FILTER    = 26,
    SO_DETACH_FILTER    = 27,

    SO_PEERNAME         = 28,

    SO_ACCEPTCONN       = 30,

    SO_PEERSEC          = 31,
    SO_PASSSEC          = 34,

    SO_MARK             = 36,

    SO_PROTOCOL         = 38,
    SO_DOMAIN           = 39,

    SO_RXQ_OVFL         = 40,

    SO_WIFI_STATUS      = 41,
    SO_PEEK_OFF         = 42,

    /* Instruct lower device to use last 4-bytes of skb data as FCS */
    SO_NOFCS            = 43,

    SO_LOCK_FILTER      = 44,

    SO_SELECT_ERR_QUEUE = 45,

    SO_BUSY_POLL        = 46,

    SO_MAX_PACING_RATE  = 47,

    SO_BPF_EXTENSIONS   = 48,

    SO_INCOMING_CPU     = 49,

    SO_ATTACH_BPF       = 50,

    SO_ATTACH_REUSEPORT_CBPF = 51,
    SO_ATTACH_REUSEPORT_EBPF = 52,

    SO_CNX_ADVICE       = 53,

    SCM_TIMESTAMPING_OPT_STATS = 54,

    SO_MEMINFO          = 55,

    SO_INCOMING_NAPI_ID = 56,

    SO_COOKIE           = 57,

    SCM_TIMESTAMPING_PKTINFO = 58,

    SO_PEERGROUPS       = 59,

    SO_ZEROCOPY         = 60,

    SO_TXTIME           = 61,

    SO_BINDTOIFINDEX    = 62,

    SO_TIMESTAMP_OLD    = 29,
    SO_TIMESTAMPNS_OLD  = 35,
    SO_TIMESTAMPING_OLD = 37,

    SO_TIMESTAMP_NEW    = 63,
    SO_TIMESTAMPNS_NEW  = 64,
    SO_TIMESTAMPING_NEW = 65,

    SO_RCVTIMEO_NEW     = 66,
    SO_SNDTIMEO_NEW     = 67,

    SO_DETACH_REUSEPORT_BPF = 68,

    SO_PREFER_BUSY_POLL = 69,
    SO_BUSY_POLL_BUDGET = 70,

    SO_NETNS_COOKIE     = 71,
    SO_BUF_LOCK         = 72,
}

impl SockOpt {
    pub const SO_GET_FILTER: Self = Self::SO_ATTACH_FILTER;
    pub const SCM_WIFI_STATUS: Self = Self::SO_WIFI_STATUS;
    pub const SO_DETACH_BPF: Self = Self::SO_DETACH_FILTER;
    pub const SCM_TXTIME: Self = Self::SO_TXTIME;
}
