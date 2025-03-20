use std::collections::VecDeque;
use std::marker::PhantomData;

use bitflags::__private::PublicFlags;
use bitflags::{
    Flags,
    bitflags,
};
use byteorder::ByteOrder;
use num_enum::{
    FromPrimitive,
    IntoPrimitive,
};
use tracing::debug;

use mogi_derive::{
    Decode,
    Encode,
    RequiredMem,
};
use mogi_utilities::encoding::{
    Decode,
    Encode,
    Encoder,
    RequiredMem,
};
use mogi_utilities::error::memory::MemoryError;

use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorError,
    FileDescriptorOps,
    FileOperationsReady,
};
use crate::emulator::abstractions::network::{
    ShutdownMethod,
    SocketAddress,
};
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::SocketDomain::{
    AF_INET,
    AF_INET6,
};
use crate::emulator::platform::linux::kernel::net::netlink::rtnetlink::nla_type::{
    IFA_ADDRESS,
    IFA_BROADCAST,
    IFA_FLAGS,
    IFA_LABEL,
    IFA_LOCAL,
};
use crate::emulator::platform::linux::kernel::net::netlink::{
    NetlinkMessageFlags,
    nlmsghdr,
};
use crate::emulator::platform::linux::kernel::net::sockopt::Sockopt;
use crate::emulator::platform::linux::kernel::socket::Optval;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug, Default)]
pub struct RTNetlinkSocket<Endianness: ByteOrder> {
    _phantom: PhantomData<Endianness>,
}

#[derive(Debug, Clone)]
pub struct NetlinkData {
    sockaddr: Option<SocketAddress>,
    messages: VecDeque<Vec<u8>>,
}

impl<Endianness: ByteOrder> RTNetlinkSocket<Endianness> {
    pub fn new() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<Endianness: ByteOrder> FileDescriptorOps for RTNetlinkSocket<Endianness> {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        todo!()
    }

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn bind(
        &self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        // TODO: What needs to be done here?
        let netlink = fd
            .netlink
            .as_mut()
            .ok_or(FileDescriptorError::NotSupported)?;
        netlink.sockaddr = Some(sockaddr);
        Ok(())
    }

    fn getsockname<'a>(
        &self,
        fd: &'a mut Self::D,
    ) -> Result<&'a SocketAddress, FileDescriptorError> {
        let netlink = fd
            .netlink
            .as_mut()
            .ok_or(FileDescriptorError::NotSupported)?;
        match &netlink.sockaddr {
            Some(s) => Ok(s),
            None => Err(FileDescriptorError::Unknown),
        }
    }

    fn sendto(
        &self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        let netlink = fd
            .netlink
            .as_mut()
            .ok_or(FileDescriptorError::NotSupported)?;
        let hdr = nlmsghdr::decode::<Endianness>(&data[..nlmsghdr::required_mem()])?;
        let payload = &data[nlmsghdr::required_mem()..];
        match hdr.nlmsg_type {
            RTNetlinkMessageType::RTM_GETADDR => {}
            _ => todo!("Only RTM_GETADDR supported currently"),
        }
        // TODO: How to handle this correctly? For now we just hard code a response according to strace of the example
        //       The payload is only 4 bytes, but the ifaddrmsg struct is 8 bytes long. Why is this the case?
        debug!("header: {:?}", hdr);
        debug!("payload: {:?}", payload);
        debug!("type: {:?}", hdr.nlmsg_type);

        let SocketAddress::Netlink { pid, .. } = &netlink.sockaddr.ok_or(FileDescriptorError::Unknown)? else {
            Err(FileDescriptorError::Unknown)?
        };

        // Loopback
        let msg = Encoder::<Endianness>::new()
            .encode(&ifaddrmsg {
                ifa_family: AF_INET as u8,
                ifa_prefixlen: 8,
                ifa_flags: 0x80,
                ifa_scope: 254,
                ifa_index: 1,
            })
            .encode_with(|encoder| encode_attr(encoder, IFA_ADDRESS, &[127, 0, 0, 1].as_slice()))
            .encode_with(|encoder| encode_attr(encoder, IFA_LOCAL, &[127, 0, 0, 1].as_slice()))
            .encode_with(|encoder| encode_attr(encoder, IFA_LABEL, &"lo\0".as_bytes()))
            .encode_with(|encoder| encode_attr(encoder, IFA_FLAGS, &ifa_flags::IFA_F_PERMANENT))
            .encode(
                &[
                    20u8, 0, 6, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0x00, 0x00, 0x00, 0xf0, 0x00,
                    0x00, 0x00,
                ]
                .as_slice(),
            );

        let hdr = nlmsghdr {
            nlmsg_len: (nlmsghdr::required_mem() + msg.encoded_length()) as u32,
            nlmsg_type: RTNetlinkMessageType::RTM_NEWADDR,
            nlmsg_flags: NetlinkMessageFlags::NLM_F_MULTI,
            nlmsg_seq: hdr.nlmsg_seq,
            nlmsg_pid: *pid, // TODO Get pid from somewhere
        };
        let msg = msg.encode_front(&hdr).finalize();
        let mut out = msg;

        // wlp7s0
        let msg = Encoder::<Endianness>::new()
            .encode(&ifaddrmsg {
                ifa_family: AF_INET as u8,
                ifa_prefixlen: 24,
                ifa_flags: 0,
                ifa_scope: 0,
                ifa_index: 4,
            })
            .encode_with(|encoder| encode_attr(encoder, IFA_ADDRESS, &[192, 168, 178, 30].as_slice()))
            .encode_with(|encoder| encode_attr(encoder, IFA_LOCAL, &[192, 168, 178, 30].as_slice()))
            .encode_with(|encoder| encode_attr(encoder, IFA_BROADCAST, &[192, 168, 178, 255].as_slice()))
            .encode_with(|encoder| encode_attr(encoder, IFA_LABEL, &"wlp7s0\0".as_bytes()))
            .encode_with(|encoder| encode_attr(encoder, IFA_FLAGS, &ifa_flags::IFA_F_NOPREFIXROUTE))
            .encode(
                &[
                    20u8, 0, 6, 0, 100, 250, 12, 0, 100, 250, 12, 0, 126, 3, 0, 0, 89, 96, 20, 0,
                ]
                .as_slice(),
            );

        let hdr = nlmsghdr {
            nlmsg_len: (nlmsghdr::required_mem() + msg.encoded_length()) as u32,
            nlmsg_type: RTNetlinkMessageType::RTM_NEWADDR,
            nlmsg_flags: NetlinkMessageFlags::NLM_F_MULTI,
            nlmsg_seq: hdr.nlmsg_seq,
            nlmsg_pid: *pid, // TODO Get pid from somewhere
        };
        let msg = msg.encode_front(&hdr).finalize();
        out.extend(msg);

        // Additional message
        // loopback
        let msg = Encoder::<Endianness>::new()
            .encode(&ifaddrmsg {
                ifa_family: AF_INET6 as u8,
                ifa_prefixlen: 128,
                ifa_flags: 0x80,
                ifa_scope: 254,
                ifa_index: 1,
            })
            .encode_with(|encoder| {
                encode_attr(encoder, IFA_ADDRESS, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1].as_slice())
            })
            .encode(
                &[
                    20u8, 0, 6, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0x00, 0x00, 0x00, 0xf0, 0x00,
                    0x00, 0x00,
                ]
                .as_slice(),
            )
            .encode_with(|encoder| encode_attr(encoder, IFA_FLAGS, &ifa_flags::IFA_F_PERMANENT));

        let hdr = nlmsghdr {
            nlmsg_len: (nlmsghdr::required_mem() + msg.encoded_length()) as u32,
            nlmsg_type: RTNetlinkMessageType::RTM_NEWADDR,
            nlmsg_flags: NetlinkMessageFlags::NLM_F_MULTI,
            nlmsg_seq: hdr.nlmsg_seq,
            nlmsg_pid: *pid, // TODO Get pid from somewhere
        };
        let mut out = msg.encode_front(&hdr).finalize();

        // wlp7s0
        let msg = Encoder::<Endianness>::new()
            .encode(&ifaddrmsg {
                ifa_family: AF_INET6 as u8,
                ifa_prefixlen: 64,
                ifa_flags: 0x01,
                ifa_scope: 0,
                ifa_index: 4,
            })
            .encode_with(|encoder| {
                encode_attr(
                    encoder,
                    IFA_ADDRESS,
                    &[32, 3, 0, 201, 127, 31, 25, 0, 211, 3, 223, 122, 219, 62, 235, 176].as_slice(),
                )
            })
            .encode(
                &[
                    20u8, 0, 6, 0, 100, 250, 12, 0, 100, 250, 12, 0, 126, 3, 0, 0, 89, 96, 20, 0,
                ]
                .as_slice(),
            )
            .encode_with(|encoder| encode_attr(encoder, IFA_FLAGS, &ifa_flags::IFA_F_SECONDARY));

        let hdr = nlmsghdr {
            nlmsg_len: (nlmsghdr::required_mem() + msg.encoded_length()) as u32,
            nlmsg_type: RTNetlinkMessageType::RTM_NEWADDR,
            nlmsg_flags: NetlinkMessageFlags::NLM_F_MULTI,
            nlmsg_seq: hdr.nlmsg_seq,
            nlmsg_pid: *pid, // TODO Get pid from somewhere
        };
        let msg = msg.encode_front(&hdr).finalize();
        out.extend(msg);

        let msg = Encoder::<Endianness>::new()
            .encode(&ifaddrmsg {
                ifa_family: AF_INET6 as u8,
                ifa_prefixlen: 64,
                ifa_flags: 0,
                ifa_scope: 0,
                ifa_index: 4,
            })
            .encode_with(|encoder| {
                encode_attr(
                    encoder,
                    IFA_ADDRESS,
                    &[32, 3, 0, 201, 127, 31, 25, 0, 15, 71, 61, 94, 29, 8, 104, 180].as_slice(),
                )
            })
            .encode(
                &[
                    20u8, 0, 6, 0, 100, 250, 12, 0, 100, 250, 12, 0, 126, 3, 0, 0, 89, 96, 20, 0,
                ]
                .as_slice(),
            )
            .encode_with(|encoder| {
                encode_attr(encoder, IFA_FLAGS, &(ifa_flags::IFA_F_MANAGETEMPADDR | ifa_flags::IFA_F_NOPREFIXROUTE))
            });

        let hdr = nlmsghdr {
            nlmsg_len: (nlmsghdr::required_mem() + msg.encoded_length()) as u32,
            nlmsg_type: RTNetlinkMessageType::RTM_NEWADDR,
            nlmsg_flags: NetlinkMessageFlags::NLM_F_MULTI,
            nlmsg_seq: hdr.nlmsg_seq,
            nlmsg_pid: *pid, // TODO Get pid from somewhere
        };
        let msg = msg.encode_front(&hdr).finalize();
        out.extend(msg);

        let msg = Encoder::<Endianness>::new()
            .encode(&ifaddrmsg {
                ifa_family: AF_INET6 as u8,
                ifa_prefixlen: 64,
                ifa_flags: ifa_flags::IFA_F_PERMANENT.bits() as u8,
                ifa_scope: 253,
                ifa_index: 4,
            })
            .encode_with(|encoder| {
                encode_attr(
                    encoder,
                    IFA_ADDRESS,
                    &[254, 128, 0, 0, 0, 0, 0, 0, 173, 11, 58, 69, 177, 184, 112, 232].as_slice(),
                )
            })
            .encode(
                &[
                    20u8, 0, 6, 0, 100, 250, 12, 0, 100, 250, 12, 0, 126, 3, 0, 0, 89, 96, 20, 0,
                ]
                .as_slice(),
            )
            .encode_with(|encoder| {
                encode_attr(encoder, IFA_FLAGS, &(ifa_flags::IFA_F_PERMANENT | ifa_flags::IFA_F_NOPREFIXROUTE))
            });

        let hdr = nlmsghdr {
            nlmsg_len: (nlmsghdr::required_mem() + msg.encoded_length()) as u32,
            nlmsg_type: RTNetlinkMessageType::RTM_NEWADDR,
            nlmsg_flags: NetlinkMessageFlags::NLM_F_MULTI,
            nlmsg_seq: hdr.nlmsg_seq,
            nlmsg_pid: *pid, // TODO Get pid from somewhere
        };
        let msg = msg.encode_front(&hdr).finalize();
        out.extend(msg);

        netlink.messages.push_front(out);

        // Final message
        let msg = Encoder::<Endianness>::new()
            .encode(&nlmsghdr {
                nlmsg_len: (nlmsghdr::required_mem() + payload.len()) as u32,
                nlmsg_type: RTNetlinkMessageType::NLMSG_DONE,
                nlmsg_flags: NetlinkMessageFlags::NLM_F_MULTI,
                nlmsg_seq: hdr.nlmsg_seq,
                nlmsg_pid: *pid,
            })
            .encode(&payload)
            .finalize();
        netlink.messages.push_front(msg);

        Ok(data.len())
    }

    fn recvmsg(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        // TODO: EAGAIN?
        let netlink = fd
            .netlink
            .as_mut()
            .ok_or(FileDescriptorError::NotSupported)?;
        let msg = netlink.messages.pop_back().unwrap_or_default();
        // debug!("{:02x?} len {:?}", &msg, msg.len());
        Ok(msg)
        // self.messages.pop_back().ok_or_else(|| GenericError::new(GenericErrorKind::Unknown, ""))
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        todo!()
    }

    fn connect(
        &self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn getsockopt(
        &self,
        fd: &mut Self::D,
        sockopt: Sockopt,
    ) -> Result<Optval, FileDescriptorError> {
        todo!()
    }

    fn accept(
        &self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        todo!()
    }

    fn shutdown(
        &self,
        fd: &mut Self::D,
        how: ShutdownMethod,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }
}

fn encode_attr<Endianness: ByteOrder, T: Encode>(
    encoder: Encoder<Endianness>,
    typ: nla_type,
    payload: &T,
) -> Encoder<Endianness> {
    let attr_size = nlattr::required_mem() + payload.encoded_size();
    let attr = nlattr {
        nla_len: attr_size as u16,
        nla_type: typ,
    };
    encoder
        .encode(&attr)
        .encode(&payload)
        .encode(&vec![0; align(attr_size) - attr_size].as_slice())
}

fn align(size: usize) -> usize {
    (size + 3) & !3
}

#[allow(non_camel_case_types)]
#[repr(u16)]
#[derive(Copy, Clone, RequiredMem)]
pub enum nla_type {
    IFA_UNSPEC,
    IFA_ADDRESS,
    IFA_LOCAL,
    IFA_LABEL,
    IFA_BROADCAST,
    IFA_ANYCAST,
    IFA_CACHEINFO,
    IFA_MULTICAST,
    IFA_FLAGS,
    IFA_RT_PRIORITY,
    /* u32, priority/metric for prefix route */
    IFA_TARGET_NETNSID,
    IFA_PROTO,
    /* u8, address protocol */
    __IFA_MAX,
}

impl Encode for nla_type {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        (*self as u16).encode::<Endianness>(buffer)
    }
}

#[derive(Encode, RequiredMem)]
#[repr(C)]
struct nlattr {
    nla_len: u16,
    nla_type: nla_type,
}

bitflags! {
    #[derive(RequiredMem)]
    #[allow(non_camel_case_types)]
    pub struct
    ifa_flags: u32 {
        const IFA_F_SECONDARY = 0x01;
        const IFA_F_NODAD = 0x02;
        const IFA_F_OPTIMISTIC = 0x04;
        const IFA_F_DADFAILED = 0x08;
        const IFA_F_HOMEADDRESS = 0x10;
        const IFA_F_DEPRECATED = 0x20;
        const IFA_F_TENTATIVE = 0x40;
        const IFA_F_PERMANENT = 0x80;
        const IFA_F_MANAGETEMPADDR = 0x100;
        const IFA_F_NOPREFIXROUTE = 0x200;
        const IFA_F_MCAUTOJOIN = 0x400;
        const IFA_F_STABLE_PRIVACY = 0x800;
    }
}

impl Encode for ifa_flags {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        self.bits().encode::<Endianness>(buffer)
    }
}

#[derive(Decode, Encode, RequiredMem)]
#[repr(C)]
struct ifaddrmsg {
    ifa_family: u8,
    ifa_prefixlen: u8,
    ifa_flags: u8,
    ifa_scope: u8,
    ifa_index: u32,
}

#[repr(u16)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Debug, FromPrimitive, IntoPrimitive, Copy, Clone, RequiredMem)]
pub enum RTNetlinkMessageType {
    NLMSG_NOOP           = 0x1,
    NLMSG_ERROR          = 0x2,
    NLMSG_DONE           = 0x3,
    NLMSG_OVERRUN        = 0x4,

    RTM_NEWLINK          = 16,
    RTM_DELLINK,
    RTM_GETLINK,
    RTM_SETLINK,

    RTM_NEWADDR          = 20,
    RTM_DELADDR,
    RTM_GETADDR,

    RTM_NEWROUTE         = 24,
    RTM_DELROUTE,
    RTM_GETROUTE,

    RTM_NEWNEIGH         = 28,
    RTM_DELNEIGH,
    RTM_GETNEIGH,

    RTM_NEWRULE          = 32,
    RTM_DELRULE,
    RTM_GETRULE,

    RTM_NEWQDISC         = 36,
    RTM_DELQDISC,
    RTM_GETQDISC,

    RTM_NEWTCLASS        = 40,
    RTM_DELTCLASS,
    RTM_GETTCLASS,

    RTM_NEWTFILTER       = 44,
    RTM_DELTFILTER,
    RTM_GETTFILTER,

    RTM_NEWACTION        = 48,
    RTM_DELACTION,
    RTM_GETACTION,

    RTM_NEWPREFIX        = 52,

    RTM_GETMULTICAST     = 58,

    RTM_GETANYCAST       = 62,

    RTM_NEWNEIGHTBL      = 64,
    RTM_GETNEIGHTBL      = 66,
    RTM_SETNEIGHTBL,

    RTM_NEWNDUSEROPT     = 68,

    RTM_NEWADDRLABEL     = 72,
    RTM_DELADDRLABEL,
    RTM_GETADDRLABEL,

    RTM_GETDCB           = 78,
    RTM_SETDCB,

    RTM_NEWNETCONF       = 80,
    RTM_DELNETCONF,
    RTM_GETNETCONF       = 82,

    RTM_NEWMDB           = 84,
    RTM_DELMDB           = 85,
    RTM_GETMDB           = 86,

    RTM_NEWNSID          = 88,
    RTM_DELNSID          = 89,
    RTM_GETNSID          = 90,

    RTM_NEWSTATS         = 92,
    RTM_GETSTATS         = 94,
    RTM_SETSTATS,

    RTM_NEWCACHEREPORT   = 96,

    RTM_NEWCHAIN         = 100,
    RTM_DELCHAIN,
    RTM_GETCHAIN,

    RTM_NEWNEXTHOP       = 104,
    RTM_DELNEXTHOP,
    RTM_GETNEXTHOP,

    RTM_NEWLINKPROP      = 108,
    RTM_DELLINKPROP,
    RTM_GETLINKPROP,

    RTM_NEWVLAN          = 112,
    RTM_DELVLAN,
    RTM_GETVLAN,

    RTM_NEWNEXTHOPBUCKET = 116,
    RTM_DELNEXTHOPBUCKET,
    RTM_GETNEXTHOPBUCKET,

    RTM_NEWTUNNEL        = 120,
    RTM_DELTUNNEL,
    RTM_GETTUNNEL,

    #[default]
    INVALID              = !0,
}

impl Encode for RTNetlinkMessageType {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        u16::from(*self).encode::<Endianness>(buffer)
    }
}

impl Decode for RTNetlinkMessageType {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        Ok(Self::from(Endianness::read_u16(val)))
    }
}
