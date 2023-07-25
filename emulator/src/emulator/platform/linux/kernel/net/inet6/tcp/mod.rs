use crate::emulator::platform::linux::kernel::net::Sockaddr;
use std::collections::VecDeque;

pub mod listener;
pub mod socket;

#[derive(Debug, Default)]
pub struct TCPData {
    pending_connections: Vec<Sockaddr>,
    pending_data: VecDeque<Vec<u8>>,
    out_data: VecDeque<Vec<u8>>,
}

impl TCPData {
    pub fn new() -> Self {
        let mut data = VecDeque::new();
        data.push_front(hex::decode("48454c463800000000000000ffff0000ffff00000000000000000000180000006f70632e7463703a2f2f6c6f63616c686f73743a34383430").unwrap());
        Self {
            pending_connections: vec![Sockaddr::IN6 {
                sin6_port: 1337,
                sin6_flowinfo: 0,
                sin6_addr: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                sin6_scope_id: 0,
            }],
            pending_data: data,
            out_data: Default::default(),
        }
    }
}
