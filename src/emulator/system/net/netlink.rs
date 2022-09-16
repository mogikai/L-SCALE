use std::collections::VecDeque;
use std::mem;

use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::system::memory::{Decode, MemoryError};
use crate::emulator::system::net::Sockaddr;

pub struct
NetlinkSocket {
    sockaddr: Option<Sockaddr>,
    messages: VecDeque<Vec<u8>>,
}


// TODO: Make global netlink handler that is referenced by netlink sockets.
//       Each kernel(task) will have access to the handler.

impl
NetlinkSocket {
    pub
    fn new() -> Self {
        Self {
            sockaddr: None,
            messages: VecDeque::new(),
        }
    }
}

impl<Endianness: ByteOrder>
Handle<Endianness> for
NetlinkSocket {
    fn bind(
        &mut self,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        // TODO: What needs to be done here?
        self.sockaddr = Some(sockaddr);
        Ok(())
    }

    fn getsockname(
        &self
    ) -> Result<&Sockaddr, GenericError> {
        match &self.sockaddr {
            Some(s) => Ok(s),
            None => Err(GenericError::new(GenericErrorKind::Unknown, ""))
        }
    }

    fn sendto(
        &mut self,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        debug!("sendto: {:?}", <nlmsghdr as Decode::<Endianness>>::decode(data));
        Ok(data.len())
    }

    fn recvmsg(
        &mut self,
        len: usize,
    ) -> Result<Vec<u8>, GenericError> {
        // TODO: EAGAIN?
        self.messages.pop_back().ok_or_else(|| GenericError::new(GenericErrorKind::Unknown, ""))
    }
}

#[derive(Debug)]
struct nlmsghdr {
    nlmsg_len: u32,    /* Length of message including header */
    nlmsg_type: u16,   /* Type of message content */
    nlmsg_flags: u16,  /* Additional flags */
    nlmsg_seq: u32,    /* Sequence number */
    nlmsg_pid: u32,    /* Sender port ID */
}

impl<Endianness: ByteOrder>
Decode<Endianness> for
nlmsghdr {
    fn decode(val: &[u8]) -> Result<Self, MemoryError> {
        Ok(Self {
            nlmsg_len: Endianness::read_u32(&val[..4]),
            nlmsg_type: Endianness::read_u16(&val[4..6]),
            nlmsg_flags: Endianness::read_u16(&val[6..8]),
            nlmsg_seq: Endianness::read_u32(&val[8..12]),
            nlmsg_pid: Endianness::read_u32(&val[12..]),
        })
    }

    fn required_mem() -> usize {
        3 * mem::size_of::<u32>() + 2 * mem::size_of::<u16>()
    }
}

#[repr(u64)]
#[allow(non_camel_case_types)]
#[derive(Debug, FromPrimitive)]
enum NetlinkFamily {
    NETLINK_ROUTE = 0,
    NETLINK_UNUSED = 1,
    NETLINK_USERSOCK = 2,
    NETLINK_FIREWALL = 3,
    NETLINK_SOCK_DIAG = 4,
    NETLINK_NFLOG = 5,
    NETLINK_XFRM = 6,
    NETLINK_SELINUX = 7,
    NETLINK_ISCSI = 8,
    NETLINK_AUDIT = 9,
    NETLINK_FIB_LOOKUP = 10,
    NETLINK_CONNECTOR = 11,
    NETLINK_NETFILTER = 12,
    NETLINK_IP6_FW = 13,
    NETLINK_DNRTMSG = 14,
    NETLINK_KOBJECT_UEVENT = 15,
    NETLINK_GENERIC = 16,
    NETLINK_SCSITRANSPORT = 18,
    NETLINK_ECRYPTFS = 19,
    NETLINK_RDMA = 20,
    NETLINK_CRYPTO = 21,
    NETLINK_SMC = 22,
    #[default]
    INVALID = !0,
}