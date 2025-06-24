use crate::emulator::platform::linux::kernel::LinuxKernelError;
use byteorder::ByteOrder;
use num_enum::{
    IntoPrimitive,
    TryFromPrimitive,
};
use tracing::{
    error,
    trace,
};

#[derive(Debug)]
pub enum Sockopt {
    Socket(Socket_Sockopt),
    Unknown,
}

impl TryFrom<(u64, u64)> for Sockopt {
    type Error = LinuxKernelError;

    fn try_from(value: (u64, u64)) -> Result<Self, Self::Error> {
        Ok(match value.0 {
            1 => Sockopt::Socket(Socket_Sockopt::try_from(value.1).map_err(|_| LinuxKernelError::EINVAL)?),
            _ => {
                trace!("Sockopt {value:?} currently not supported");
                Sockopt::Unknown
            }
        })
    }
}

#[allow(non_camel_case_types)]
#[derive(TryFromPrimitive, IntoPrimitive, Clone, Copy, Debug)]
#[repr(u64)]
pub enum Socket_Sockopt {
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
    SO_SECURITY_AUTHENTICATION = 22,
    SO_SECURITY_ENCRYPTION_TRANSPORT = 23,
    SO_SECURITY_ENCRYPTION_NETWORK = 24,
    SO_BINDTODEVICE     = 25,
    SO_ATTACH_FILTER    = 26,
    SO_DETACH_FILTER    = 27,
    // SO_GET_FILTER       = SO_ATTACH_FILTER,
    SO_PEERNAME         = 28,
    SO_ACCEPTCONN       = 30,
    SO_PEERSEC          = 31,
    SO_PASSSEC          = 34,
    SO_MARK             = 36,
    SO_PROTOCOL         = 38,
    SO_DOMAIN           = 39,
    SO_RXQ_OVFL         = 40,
    SO_WIFI_STATUS      = 41,
    // SCM_WIFI_STATUS     = SO_WIFI_STATUS,
    SO_PEEK_OFF         = 42,
    SO_NOFCS            = 43,
    SO_LOCK_FILTER      = 44,
    SO_SELECT_ERR_QUEUE = 45,
    SO_BUSY_POLL        = 46,
    SO_MAX_PACING_RATE  = 47,
    SO_BPF_EXTENSIONS   = 48,
    SO_INCOMING_CPU     = 49,
    SO_ATTACH_BPF       = 50,
    // SO_DETACH_BPF       = SO_DETACH_FILTER,
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
    // SCM_TXTIME          = SO_TXTIME,
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
    SO_RESERVE_MEM      = 73,
    SO_TXREHASH         = 74,
    SO_RCVMARK          = 75,
    SO_PASSPIDFD        = 76,
    SO_PEERPIDFD        = 77,
}
