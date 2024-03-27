use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use mogi_system::components::restore::Restorable;
use mogi_system::errors::generic::GenericError;
use std::ops::Add;

use crate::emulator::platform::linux::kernel::eventpoll::EpollData;
use crate::emulator::platform::linux::kernel::fs::FileData;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::TCPData;
use crate::emulator::platform::linux::kernel::net::inet6::udp::UDPData;
use crate::emulator::platform::linux::kernel::net::netlink::rtnetlink::NetlinkData;
use crate::emulator::platform::linux::kernel::net::unix::UnixSocketData;
use crate::emulator::platform::linux::signalfd::SignalFD;
use crate::emulator::system::fd_map::FdId;

// TODO: Move to platform directory and make FileDescriptor generic
#[derive(Default, Debug, Clone)]
pub struct FileDescriptorData {
    pub file: Option<FileData>,
    pub signals: Option<SignalFD>,
    pub tcp: Option<TCPData>,
    pub udp: Option<UDPData>,
    pub epoll: Option<EpollData>,
    pub usock: Option<UnixSocketData>,
    pub netlink: Option<NetlinkData>,
}

#[derive(Hash, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default)]
pub struct FD(i64);

impl FdId for FD {}

impl Add<i32> for FD {
    type Output = FD;

    fn add(
        self,
        rhs: i32,
    ) -> Self::Output {
        FD(self.0 + i64::from(rhs))
    }
}

impl From<i32> for FD {
    fn from(value: i32) -> Self {
        Self(value.into())
    }
}

impl From<u64> for FD {
    fn from(value: u64) -> Self {
        FD(value as i64)
    }
}

impl From<FD> for usize {
    fn from(value: FD) -> Self {
        value.0 as usize
    }
}

impl From<FD> for u64 {
    fn from(value: FD) -> Self {
        value.0 as u64
    }
}
