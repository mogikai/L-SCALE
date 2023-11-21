use std::cell::RefCell;
use std::collections::{
    BTreeMap,
    VecDeque,
};
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

use fuzzyemu::emulator::abstractions::network::{
    NetworkError,
    SocketId,
};
use fuzzyemu::emulator::interface::network::tcp::{
    TCPSocket,
    TCPSystemControl,
};

#[derive(Debug, Default)]
pub struct TCPStreamObservation {
    /// Contains a double ended queue of outputs for each tcp stream.
    /// New outputs are added to the back of the Deque.
    outgoing_data: BTreeMap<SocketId, VecDeque<Vec<u8>>>,
}

#[derive(Debug)]
pub struct TCPStreamObserver<TcpT>
where
    TcpT: TCPSystemControl,
{
    tcp: Rc<RefCell<TcpT>>,
    observation: TCPStreamObservation,
}

impl<TcpT> TCPStreamObserver<TcpT>
where
    TcpT: TCPSystemControl,
{
    pub fn new(tcp: &Rc<RefCell<TcpT>>) -> Self {
        Self {
            tcp: tcp.clone(),
            observation: Default::default(),
        }
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
        let mut tcp = self.tcp.borrow_mut();
        self.observation.outgoing_data = tcp
            .sockets_mut()
            .map(|(&id, socket)| Ok((id, socket.take_output()?)))
            .try_collect()
            .map_err(|e: NetworkError| Error::unknown(""))?;
        Ok(())
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
