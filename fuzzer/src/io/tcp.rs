use std::cell::RefCell;
use std::cmp::min;
use std::collections::VecDeque;

use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::file_descriptors::FileOperationsReady;
use fuzzyemu::emulator::abstractions::network::tcp_system::TCPSystem;
use fuzzyemu::emulator::abstractions::network::{NetworkError, SocketAddress, SocketId};
use fuzzyemu::emulator::abstractions::restore::Restorable;
use fuzzyemu::util::auto_key::lazy_clone_map::LazyCloneMap;
use fuzzyemu::util::rcow::RCow;

#[derive(Debug)]
pub struct TCP {
    sockets: LazyCloneMap<TCPSocket>,
}

#[derive(Debug, Clone)]
pub struct TCPSocket {
    // TODO: Maybe we can use a restorable data structure here, instead of cloning the whole thing?
    incoming: RCow<VecDeque<u8>>,
    outgoing: RCow<VecDeque<Vec<u8>>>,
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

    last_interest: RCow<RefCell<FileOperationsReady>>,
}

impl TCPSocket {
    pub fn poll(
        &self,
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
        *self.last_interest.borrow_mut() = interest.difference(ops_ready);
        ops_ready
    }

    pub fn interest(&self) -> FileOperationsReady {
        *self.last_interest.borrow()
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

impl TCP {
    pub fn new() -> Self {
        TCP {
            sockets: Default::default(),
        }
    }

    /// Register a new incoming connection on the listener socket identified by `listener`
    pub fn new_connection(
        &mut self,
        listener: &SocketId,
        remote_address: SocketAddress,
    ) -> Result<SocketId, NetworkError> {
        let socket = {
            let listener = self.sockets.get(&listener).ok_or(NetworkError::Unknown)?;
            TCPSocket {
                incoming: Default::default(),
                outgoing: Default::default(),
                pending_connections: RCow::new(None),
                local_address: RCow::new(Some(listener.local_address.ok_or(NetworkError::Unknown)?)),
                remote_address: RCow::new(Some(remote_address)),
                last_interest: RCow::new(RefCell::new(FileOperationsReady::empty())),
            }
        };
        let id = self.sockets.insert(socket);
        self.sockets
            .get_mut(&listener)
            .unwrap()
            .register_connection(id)?;
        Ok(id)
    }

    pub fn socket(
        &self,
        id: SocketId,
    ) -> Option<&TCPSocket> {
        self.sockets.get(&id)
    }

    pub fn socket_mut(
        &mut self,
        id: SocketId,
    ) -> Option<&mut TCPSocket> {
        self.sockets.get_mut(&id)
    }

    pub fn sockets(&self) -> impl Iterator<Item = (&SocketId, &TCPSocket)> {
        self.sockets.iter()
    }

    pub fn sockets_mut(&mut self) -> impl Iterator<Item = (&SocketId, &mut TCPSocket)> {
        self.sockets.iter_mut()
    }

    pub fn push_data(
        &mut self,
        id: SocketId,
        data: &[u8],
    ) -> Result<(), NetworkError> {
        self.sockets
            .get_mut(&id)
            .ok_or(NetworkError::Unknown)?
            .incoming
            .extend(data);
        Ok(())
    }
}

impl TCPSystem for TCP {
    fn new_socket(&mut self) -> Result<SocketId, NetworkError> {
        Ok(self.sockets.insert(TCPSocket {
            incoming: Default::default(),
            outgoing: Default::default(),
            pending_connections: RCow::new(None),
            local_address: RCow::new(None),
            remote_address: RCow::new(None),
            last_interest: RCow::new(RefCell::new(FileOperationsReady::empty())),
        }))
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

    fn poll(
        &self,
        id: SocketId,
        interest: FileOperationsReady,
    ) -> Result<FileOperationsReady, NetworkError> {
        let sock = self.sockets.get(&id).ok_or(NetworkError::Unknown)?;
        Ok(sock.poll(interest))
    }

    fn send(
        &mut self,
        id: SocketId,
        data: &[u8],
    ) -> Result<usize, NetworkError> {
        let mut sock = self.sockets.get_mut(&id).ok_or(NetworkError::Unknown)?;
        sock.outgoing.push_front(data.to_vec());
        Ok(data.len())
    }

    fn recvfrom(
        &mut self,
        id: SocketId,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), NetworkError> {
        let mut sock = self.sockets.get_mut(&id).ok_or(NetworkError::Unknown)?;
        let len = min(sock.incoming.len(), len);
        let data = sock.incoming.drain(0..len).collect();
        let addr = sock.remote_address.unwrap();
        Ok((data, addr))
    }
}

impl Restorable for TCP {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.sockets.snapshot()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.sockets.restore()
    }
}
