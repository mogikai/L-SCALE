use std::mem;

use bitflags::bitflags;
use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};
use crate::emulator::abstractions::mmu::MemoryError;
use crate::emulator::system::net::netlink::rtnetlink::{RTNetlinkMessageType, RTNetlinkSocket};

pub mod rtnetlink;

// TODO: Make global netlink handler that is referenced by netlink sockets.
//       Each kernel(task) will have access to the handler.

pub fn from_protocol<Endianness: ByteOrder + 'static>(protocol: u64) -> Result<Box<dyn FileDescriptorOps>, GenericError> {
    use NetlinkFamily::*;
    Ok(match NetlinkFamily::from(protocol) {
        NETLINK_ROUTE => Box::new(RTNetlinkSocket::<Endianness>::new()),
        _ => todo!(),
    })
}

#[derive(Debug)]
#[repr(C)]
struct nlmsghdr {
    nlmsg_len: u32,
    /* Length of message including header */
    nlmsg_type: RTNetlinkMessageType,
    /* Type of message content */
    nlmsg_flags: NetlinkMessageFlags,
    /* Additional flags */
    nlmsg_seq: u32,
    /* Sequence number */
    nlmsg_pid: u32,
    /* Sender port ID */
}

impl Decode for nlmsghdr {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        Ok(Self {
            nlmsg_len: Endianness::read_u32(&val[..4]),
            nlmsg_type: RTNetlinkMessageType::from(Endianness::read_u16(&val[4..6])),
            nlmsg_flags: NetlinkMessageFlags::from_bits(Endianness::read_u16(&val[6..8]))
                .ok_or(MemoryError::ConversionError)?,
            nlmsg_seq: Endianness::read_u32(&val[8..12]),
            nlmsg_pid: Endianness::read_u32(&val[12..]),
        })
    }
}

impl RequiredMem for nlmsghdr {}

impl Encode for nlmsghdr {
    fn encode<Endianness: ByteOrder>(&self) -> Vec<u8> {
        let mut out = vec![0; mem::size_of::<Self>()];
        Endianness::write_u32(&mut out[..4], self.nlmsg_len);
        Endianness::write_u16(&mut out[4..6], self.nlmsg_type.into());
        Endianness::write_u16(&mut out[6..8], self.nlmsg_flags.bits());
        Endianness::write_u32(&mut out[8..12], self.nlmsg_seq);
        Endianness::write_u32(&mut out[12..], self.nlmsg_pid);
        out
    }
}

bitflags! {
    #[derive(Debug)]
    struct
    NetlinkMessageFlags: u16 {
        const NLM_F_REQUEST         = 0x01;	    /* It is request message. 	*/
        const NLM_F_MULTI           = 0x02;	    /* Multipart message, terminated by NLMSG_DONE */
        const NLM_F_ACK             = 0x04;	    /* Reply with ack, with zero or error code */
        const NLM_F_ECHO            = 0x08;	    /* Echo this request 		*/
        const NLM_F_DUMP_INTR       = 0x10;	    /* Dump was inconsistent due to sequence change */
        const NLM_F_DUMP_FILTERED   = 0x20;	    /* Dump was filtered as requested */

        /* Modifiers to GET request */
        const NLM_GET_F_ROOT        = 0x100;	/* specify tree	root	*/
        const NLM_GET_F_MATCH       = 0x200;	/* return all matching	*/
        const NLM_GET_F_ATOMIC      = 0x400;	/* atomic GET		*/
        const NLM_GET_F_DUMP        = Self::NLM_GET_F_ROOT.bits() | Self::NLM_GET_F_MATCH.bits();

        /* Modifiers to NEW request */
        const NLM_NEW_F_REPLACE     = 0x100;	/* Override existing		*/
        const NLM_NEW_F_EXCL        = 0x200;	/* Do not touch, if it exists	*/
        const NLM_NEW_F_CREATE      = 0x400;	/* Create, if it does not exist	*/
        const NLM_NEW_F_APPEND      = 0x800;	/* Add to end of list		*/

        /* Modifiers to DELETE request */
        const NLM_DELETE_F_NONREC    = 0x100;	/* Do not delete recursively	*/
        const NLM_DELETE_F_BULK      = 0x200;	/* Delete multiple objects	*/

        /* Flags for ACK message */
        const NLM_ACK_F_CAPPED       = 0x100;	/* request was capped */
        const NLM_ACK_F_ACK_TLVS     = 0x200;	/* extended ACK TVLs were included */
    }
}

#[repr(u64)]
#[allow(non_camel_case_types)]
#[derive(Debug, FromPrimitive)]
enum NetlinkFamily {
    NETLINK_ROUTE      = 0,
    NETLINK_UNUSED     = 1,
    NETLINK_USERSOCK   = 2,
    NETLINK_FIREWALL   = 3,
    NETLINK_SOCK_DIAG  = 4,
    NETLINK_NFLOG      = 5,
    NETLINK_XFRM       = 6,
    NETLINK_SELINUX    = 7,
    NETLINK_ISCSI      = 8,
    NETLINK_AUDIT      = 9,
    NETLINK_FIB_LOOKUP = 10,
    NETLINK_CONNECTOR  = 11,
    NETLINK_NETFILTER  = 12,
    NETLINK_IP6_FW     = 13,
    NETLINK_DNRTMSG    = 14,
    NETLINK_KOBJECT_UEVENT = 15,
    NETLINK_GENERIC    = 16,
    NETLINK_SCSITRANSPORT = 18,
    NETLINK_ECRYPTFS   = 19,
    NETLINK_RDMA       = 20,
    NETLINK_CRYPTO     = 21,
    NETLINK_SMC        = 22,
    #[default]
    INVALID            = !0,
}
