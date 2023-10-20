use std::fmt::Debug;
use std::hash::Hash;

use thiserror::Error;

use crate::util::auto_key::AutoKey;

pub mod tcp_system;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Unknown network error")]
    Unknown,
}

pub type SocketId = AutoKey;

#[derive(Debug, Copy, Clone)]
pub enum IPAddress {
    V6 {
        port: u16,
        flowinfo: u32,
        addr: [u8; 16],
        scope_id: u32,
    },
    V4 {
        port: u16,
        addr: u32,
    },
}

#[derive(Debug, Copy, Clone)]
pub enum SocketAddress {
    IP(IPAddress),
    Netlink { pad: u16, pid: u32, groups: u32 },
    Unix { path: [u8; 108] },
    Unspecified { data: [u8; 14] },
}
