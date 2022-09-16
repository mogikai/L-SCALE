use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::handles::Handle;
use crate::emulator::platform::linux::kernel::LinuxKernelError;
use crate::emulator::system::memory::{Decode, Encode, MemoryError};

pub mod netlink;

pub struct
Socket {}

impl
Socket {
    fn new() -> Self {
        Self {}
    }
}

impl<Endianness: ByteOrder>
Handle<Endianness> for
Socket {
}

pub enum
Sockaddr {
    IN { sin_port: u16, sin_addr: u32 },
    NL { nl_pad: u16, nl_pid: u32, nl_groups: u32 },
    IN6,
}

impl<Endianness: ByteOrder>
Decode<Endianness> for
Sockaddr {
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
            _ => Err(MemoryError::ConversionError)?,
        })
    }

    fn required_mem() -> usize {
        128 // Defined in kernel
    }
}

impl<Endianness: ByteOrder>
Encode<Endianness> for
Sockaddr {
    fn encode(&self) -> Vec<u8> {
        let out = vec![0; 128];
        out
    }
}

pub struct
SocketFlags {
    flags: u64,
}

impl
SocketFlags {
    fn SOCK_CLOEXEC(
        &self
    ) -> bool {
        self.flags & 0x02000000 != 0
    }

    fn SOCK_NONBLOCK(
        &self
    ) -> bool {
        self.flags & 0x00004000 != 0
    }
}

impl
From<u64> for
SocketFlags {
    fn from(val: u64) -> Self {
        Self {
            flags: val & !0xFFF
        }
    }
}

#[derive(Debug)]
pub enum
SocketType {
    SOCK_STREAM = 1,
    SOCK_DGRAM = 2,
    SOCK_RAW = 3,
    SOCK_RDM = 4,
    SOCK_SEQPACKET = 5,
    SOCK_DCCP = 6,
    SOCK_PACKET = 10,
}

impl
TryFrom<u64> for
SocketType {
    type Error = LinuxKernelError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        use SocketType::*;
        Ok(match value & 0xFFF {
            1 => SOCK_STREAM,
            2 => SOCK_DGRAM,
            3 => SOCK_RAW,
            4 => SOCK_RDM,
            5 => SOCK_SEQPACKET,
            6 => SOCK_DCCP,
            10 => SOCK_PACKET,
            _ => Err(LinuxKernelError::EINVAL)?
        })
    }
}

#[repr(u64)]
#[allow(non_camel_case_types)]
#[derive(Debug, FromPrimitive)]
pub enum
SocketDomain {
    #[default]
    AF_UNSPEC = 0,
    AF_UNIX = 1,
    AF_INET = 2,
    AF_AX25 = 3,
    AF_IPX = 4,
    AF_APPLETALK = 5,
    AF_NETROM = 6,
    AF_BRIDGE = 7,
    AF_ATMPVC = 8,
    AF_X25 = 9,
    AF_INET6 = 10,
    AF_ROSE = 11,
    AF_DECnet = 12,
    AF_NETBEUI = 13,
    AF_SECURITY = 14,
    AF_KEY = 15,
    AF_NETLINK = 16,
    AF_PACKET = 17,
    AF_ASH = 18,
    AF_ECONET = 19,
    AF_ATMSVC = 20,
    AF_RDS = 21,
    AF_SNA = 22,
    AF_IRDA = 23,
    AF_PPPOX = 24,
    AF_WANPIPE = 25,
    AF_LLC = 26,
    AF_CAN = 29,
    AF_TIPC = 30,
    AF_BLUETOOTH = 31,
    AF_IUCV = 32,
    AF_RXRPC = 33,
    AF_ISDN = 34,
    AF_PHONET = 35,
    AF_IEEE802154 = 36,
    AF_CAIF = 37,
    AF_ALG = 38,
}