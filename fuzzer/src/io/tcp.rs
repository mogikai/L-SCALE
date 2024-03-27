use std::cmp::min;
use std::collections::VecDeque;
use std::mem::swap;

use mogi_emulator::emulator::abstractions::file_descriptors::FileOperationsReady;
use mogi_emulator::emulator::abstractions::network::tcp_system::TCPSystem;
use mogi_emulator::emulator::abstractions::network::{
    NetworkError,
    ShutdownMethod,
    SocketAddress,
    SocketId,
};
use mogi_emulator::emulator::abstractions::restore::Restorable;
use mogi_emulator::emulator::interface::network::tcp::{
    StreamId,
    TCPSocket,
    TCPSystemControl,
};
use mogi_emulator::util::auto_key::lazy_clone_map::LazyCloneMap;
use mogi_emulator::util::rcow::RCow;
use mogi_system::errors::generic::{
    GenericError,
    GenericErrorKind,
};

#[derive(Debug, Default)]
pub struct TCP {
    sockets: LazyCloneMap<TCPSocketImpl>,
    snapshots: Vec<LazyCloneMap<TCPSocketImpl>>,
}

#[derive(Debug, Clone)]
struct Buffer<T: Clone> {
    buffer: VecDeque<T>,
    open: bool,
}

impl<T: Clone> Default for Buffer<T> {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            open: true,
        }
    }
}

impl<T: Clone> Buffer<T> {
    fn close(&mut self) {
        self.open = false;
    }

    fn open(&mut self) {
        self.open = true;
    }

    fn is_open(&self) -> bool {
        self.open
    }

    fn write(
        &mut self,
        values: &[T],
    ) -> Result<(), NetworkError> {
        if !self.open {
            Err(NetworkError::Closed)?
        }
        Ok(self.buffer.extend(values.iter().cloned()))
    }

    fn write_one(
        &mut self,
        value: T,
    ) -> Result<(), NetworkError> {
        if !self.open {
            Err(NetworkError::Closed)?
        }
        Ok(self.buffer.push_back(value))
    }

    fn read(
        &mut self,
        n: usize,
    ) -> Result<Vec<T>, NetworkError> {
        if !self.open {
            Err(NetworkError::Closed)?
        }
        let n = min(self.buffer.len(), n);
        Ok(self.buffer.drain(0..n).collect())
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

impl<T: Clone> From<Buffer<T>> for VecDeque<T> {
    fn from(value: Buffer<T>) -> Self {
        value.buffer
    }
}

impl<T: Clone> AsRef<VecDeque<T>> for Buffer<T> {
    fn as_ref(&self) -> &VecDeque<T> {
        &self.buffer
    }
}

#[derive(Debug, Clone, Default)]
pub struct TCPSocketImpl {
    // TODO: Maybe we can use a restorable data structure here, instead of cloning the whole thing?
    incoming: RCow<Buffer<u8>>,
    outgoing: RCow<Buffer<Vec<u8>>>,
    /// If the socket is listening, it has a vector of incoming connections.
    /// The connections will already have a new socket associated with them
    /// in the socket map. We simply store the new ids here.
    pending_connections: RCow<Option<VecDeque<SocketId>>>,

    /// The socket may not have an address assigned to it initially.
    /// If it is created by the application with socket() then the
    /// address is only assigned once bind() is called
    local_address: RCow<Option<SocketAddress>>,

    /// Sockets that are fully connected have both a socket and a remote address.
    /// Listeners only have a socket address.
    remote_address: RCow<Option<SocketAddress>>,

    /// Previous poll operations had interest in these operations, but couldn't
    /// fulfil them the last time.
    unfulfilled_interest: RCow<FileOperationsReady>,
}

impl TCPSocketImpl {
    pub fn shutdown(
        &mut self,
        how: ShutdownMethod,
    ) -> Result<(), NetworkError> {
        match how {
            ShutdownMethod::ShutdownReadEnd => self.incoming.close(),
            ShutdownMethod::ShutdownWriteEnd => self.outgoing.close(),
            ShutdownMethod::ShutdownBoth => {
                self.incoming.close();
                self.outgoing.close();
            }
        }
        Ok(())
    }

    pub fn poll(
        &mut self,
        interest: FileOperationsReady,
    ) -> FileOperationsReady {
        let mut ops_ready = FileOperationsReady::empty();
        if !self.incoming.is_empty() {
            ops_ready |= FileOperationsReady::Read;
        }
        if let Some(pending) = self.pending_connections.as_ref() {
            if !pending.is_empty() {
                ops_ready |= FileOperationsReady::Read;
            }
        }
        // Save the interest bits that cannot already be satisfied.
        *self.unfulfilled_interest = (interest | *self.unfulfilled_interest).difference(ops_ready);
        ops_ready
    }

    pub fn interest(&self) -> FileOperationsReady {
        *self.unfulfilled_interest
    }

    pub fn address(&self) -> Option<SocketAddress> {
        *self.local_address
    }

    pub fn is_listener(&self) -> bool {
        self.pending_connections.is_some()
    }

    /// Registers a new connection for the listener socket.
    /// If the socket is not a listener, this method will return an error.
    pub fn register_connection(
        &mut self,
        socket_id: SocketId,
    ) -> Result<(), NetworkError> {
        if let Some(pending) = &mut *self.pending_connections {
            pending.push_front(socket_id);
            Ok(())
        } else {
            Err(NetworkError::Unknown)?
        }
    }
}

impl TCPSocket for TCPSocketImpl {
    fn interest(&self) -> FileOperationsReady {
        *self.unfulfilled_interest
    }

    fn bound_address(&self) -> Option<SocketAddress> {
        *self.local_address
    }

    fn stream_id(&self) -> StreamId {
        todo!()
    }

    fn is_listener(&self) -> bool {
        self.pending_connections.is_some()
    }

    fn push_data(
        &mut self,
        data: &[u8],
    ) -> Result<(), NetworkError> {
        self.incoming.write(data)
    }

    fn take_output(&mut self) -> Result<VecDeque<Vec<u8>>, NetworkError> {
        let mut result = Default::default();
        swap(&mut result, &mut *self.outgoing);
        Ok(result.into())
    }

    fn peek_output(&self) -> Result<&VecDeque<Vec<u8>>, NetworkError> {
        Ok(self.outgoing.as_ref())
    }
}

impl TCP {
    pub fn new() -> Self {
        TCP {
            sockets: Default::default(),
            snapshots: vec![],
        }
    }
}

impl TCPSystemControl for TCP {
    fn sockets(&self) -> impl Iterator<Item = (&SocketId, &impl TCPSocket)> {
        self.sockets.iter()
    }

    fn sockets_mut(&mut self) -> impl Iterator<Item = (&SocketId, &mut impl TCPSocket)> {
        self.sockets.iter_mut()
    }

    fn new_connection(
        &mut self,
        listener: &SocketId,
        remote_address: SocketAddress,
    ) -> Result<SocketId, NetworkError> {
        let socket = {
            let listener = self.sockets.get(listener).ok_or(NetworkError::Unknown)?;
            TCPSocketImpl {
                incoming: Default::default(),
                outgoing: Default::default(),
                pending_connections: RCow::new(None),
                local_address: RCow::new(Some(listener.local_address.ok_or(NetworkError::Unknown)?)),
                remote_address: RCow::new(Some(remote_address)),
                unfulfilled_interest: RCow::new(FileOperationsReady::empty()),
            }
        };
        let id = self.sockets.insert(socket);
        self.sockets
            .get_mut(listener)
            .unwrap()
            .register_connection(id)?;
        Ok(id)
    }

    fn socket(
        &self,
        id: SocketId,
    ) -> Option<&impl TCPSocket> {
        self.sockets.get(&id)
    }

    fn socket_mut(
        &mut self,
        id: SocketId,
    ) -> Option<&mut impl TCPSocket> {
        self.sockets.get_mut(&id)
    }
}

impl TCPSystem for TCP {
    fn new_socket(&mut self) -> Result<SocketId, NetworkError> {
        Ok(self.sockets.insert(TCPSocketImpl::default()))
    }

    fn bind(
        &mut self,
        id: SocketId,
        address: SocketAddress,
    ) -> Result<(), NetworkError> {
        // TODO: Look up if the address is already bound to different socket.
        let local_addr = &mut self
            .sockets
            .get_mut(&id)
            .ok_or(NetworkError::Unknown)?
            .local_address;
        if local_addr.is_some() {
            Err(NetworkError::Unknown)?
        }
        **local_addr = Some(address);
        Ok(())
    }

    fn close(
        &mut self,
        id: SocketId,
    ) -> Result<(), NetworkError> {
        self.sockets.remove(&id);
        Ok(())
    }

    fn accept(
        &mut self,
        id: SocketId,
    ) -> Result<(SocketId, SocketAddress), NetworkError> {
        let id = self
            .sockets
            .get_mut(&id)
            .ok_or(NetworkError::Unknown)?
            .pending_connections
            .as_mut()
            .ok_or(NetworkError::Unknown)?
            .pop_front()
            .ok_or(NetworkError::Unknown)?;
        let addr = self
            .sockets
            .get(&id)
            .expect("The socket id needs to exist")
            .remote_address
            .ok_or(NetworkError::Unknown)?;
        Ok((id, addr))
    }

    fn listen(
        &mut self,
        id: SocketId,
        backlog: usize,
    ) -> Result<(), NetworkError> {
        *self
            .sockets
            .get_mut(&id)
            .ok_or(NetworkError::Unknown)?
            .pending_connections = Some(VecDeque::with_capacity(backlog));
        Ok(())
    }

    fn shutdown(
        &mut self,
        id: SocketId,
        how: ShutdownMethod,
    ) -> Result<(), NetworkError> {
        let sock = self.sockets.get_mut(&id).ok_or(NetworkError::Unknown)?;
        sock.shutdown(how)
    }

    fn poll(
        &mut self,
        id: SocketId,
        interest: FileOperationsReady,
    ) -> Result<FileOperationsReady, NetworkError> {
        let sock = self.sockets.get_mut(&id).ok_or(NetworkError::Unknown)?;
        Ok(sock.poll(interest))
    }

    fn send(
        &mut self,
        id: SocketId,
        data: &[u8],
    ) -> Result<usize, NetworkError> {
        let sock = self.sockets.get_mut(&id).ok_or(NetworkError::Unknown)?;
        sock.outgoing.write_one(data.to_vec())?;
        Ok(data.len())
    }

    fn recvfrom(
        &mut self,
        id: SocketId,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), NetworkError> {
        let sock = self.sockets.get_mut(&id).ok_or(NetworkError::Unknown)?;
        let addr = sock.remote_address.unwrap();
        let data = sock.incoming.read(len)?;
        Ok((data, addr))
    }
}

impl Restorable for TCP {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(self.sockets.clone());
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        if self.snapshots.len() <= 1 {
            Err(GenericError::new(GenericErrorKind::Unknown, "No previous snapshot available"))?;
        }
        self.snapshots.pop();
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        self.sockets
            .clone_from(self.snapshots.last().unwrap_or(&Default::default()));
        Ok(())
    }
}
