use std::collections::VecDeque;

use crate::emulator::platform::linux::kernel::net::Sockaddr;

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
        data.push_front(hex::decode("4f504e4684000000000000002f000000687474703a2f2f6f7063666f756e646174696f6e2e6f72672f55412f5365637572697479506f6c696379234e6f6e65ffffffffffffffff01000000010000000100be0100005d738f5c000000000000000000000000ffffffff00000000000000000000000000000001000000ffffffffc0270900").unwrap());
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
