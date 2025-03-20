use crate::emulator::abstractions::network::SocketId;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{
    Debug,
    Formatter,
};
use std::rc::Rc;

use crate::emulator::platform::linux::kernel::net::Sockaddr;

pub mod socket;

#[derive(Clone)]
pub struct TCPData {
    tcp_system: Rc<RefCell<dyn TCPSystem>>,
    id: SocketId,
}

impl Debug for TCPData {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("TCPData").field("id", &self.id).finish()
    }
}

impl TCPData {
    pub fn new(
        tcp_system: Rc<RefCell<dyn TCPSystem>>,
        id: SocketId,
    ) -> Self {
        Self { tcp_system, id }
    }
}
