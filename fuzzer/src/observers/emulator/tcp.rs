use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

use libafl::executors::ExitKind;
use libafl::inputs::UsesInput;
use libafl::observers::Observer;
use libafl_bolts::{
    Error,
    Named,
};
use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
};

use fuzzyemu::emulator::interface::network::tcp::TCPSystemControl;

#[derive(Debug)]
pub struct TCPStreamObserver<TcpT>
where
    TcpT: TCPSystemControl,
{
    tcp: Rc<RefCell<TcpT>>,
}

impl<TcpT> TCPStreamObserver<TcpT>
where
    TcpT: TCPSystemControl,
{
    pub fn new(tcp: &Rc<RefCell<TcpT>>) -> Self {
        Self { tcp: tcp.clone() }
    }
}

impl<TcpT> Named for TCPStreamObserver<TcpT>
where
    TcpT: TCPSystemControl,
{
    fn name(&self) -> &str {
        "TCPStreamObserver"
    }
}

impl<StateT, TcpT> Observer<StateT> for TCPStreamObserver<TcpT>
where
    StateT: UsesInput,
    TcpT: TCPSystemControl,
{
    fn post_exec(
        &mut self,
        _state: &mut StateT,
        _input: &StateT::Input,
        _exit_kind: &ExitKind,
    ) -> Result<(), Error> {
        // TODO: Gather
        self.tcp.borrow_mut();
        todo!()
    }
}

impl<TcpT> Serialize for TCPStreamObserver<TcpT>
where
    TcpT: TCPSystemControl,
{
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}

impl<'de, TcpT> Deserialize<'de> for TCPStreamObserver<TcpT>
where
    TcpT: TCPSystemControl,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}
