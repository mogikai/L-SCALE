use std::cell::RefCell;
use std::collections::{
    BTreeMap,
    VecDeque,
};
use std::fmt::Debug;
use std::rc::Rc;

use mogi_emulator::emulator::abstractions::network::SocketId;
use mogi_emulator::emulator::interface::network::tcp::TCPSystemControl;

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
// impl<StateT, TcpT> Observer<StateT> for TCPStreamObserver<TcpT>
// where
//     StateT: UsesInput,
//     TcpT: TCPSystemControl,
// {
//     fn post_exec(
//         &mut self,
//         _state: &mut StateT,
//         _input: &StateT::Input,
//         _exit_kind: &ExitKind,
//     ) -> Result<(), Error> {
//         let mut tcp = self.tcp.borrow_mut();
//         self.observation.outgoing_data = tcp
//             .sockets_mut()
//             .map(|(&id, socket)| Ok((id, socket.take_output()?)))
//             .try_collect()
//             .map_err(|e: NetworkError| Error::unknown(""))?;
//         Ok(())
//     }
// }
