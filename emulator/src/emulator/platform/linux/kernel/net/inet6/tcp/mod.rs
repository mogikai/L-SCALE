use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::network::SocketId;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::emulator::platform::linux::kernel::net::Sockaddr;

pub mod socket;

#[derive(Debug)]
pub struct TCPData {
    tcp_system: Rc<RefCell<dyn TCPSystem>>,
    id: SocketId,
}

impl TCPData {
    pub fn new(
        tcp_system: Rc<RefCell<dyn TCPSystem>>,
        id: SocketId,
    ) -> Self {
        Self { tcp_system, id }
    }
}
