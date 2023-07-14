use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::kernel::eventpoll::Epoll;
use crate::emulator::platform::linux::signalfd::SignalFD;
use crate::emulator::system::filesystem::file_descriptor::FileData;
use crate::emulator::system::net::inet6::tcp::TcpSock;
use crate::emulator::system::net::unix::UnixSocketData;

#[derive(Debug)]
pub struct FileDescriptor {
    /// The data field contains all state data for the fds. Each system component has
    /// its own optional field in the data struct. The HandleOps implementation
    /// is responsible for checking whether the appropriate data field exists.
    /// If it does not, the user space application called the operation on an invalid
    /// handle, and the HandleOps implementation shall return an EBADF error.
    pub data: FileDescriptorData,
    /// The ops implementation may contain pointers to system components.
    /// It must however be stateless. All data pertaining to state or
    /// similar fd related info must be contained in the data field.
    pub ops: Box<dyn FileDescriptorOps>,
}

#[derive(Default, Debug)]
pub struct FileDescriptorData {
    pub file: Option<FileData>,
    pub signals: Option<SignalFD>,
    pub socket: Option<TcpSock>,
    pub epoll: Option<Epoll>,
    pub usock: Option<UnixSocketData>,
}

impl FileDescriptor {
    pub fn new(ops: Box<dyn FileDescriptorOps>) -> Self {
        FileDescriptor {
            data: Default::default(),
            ops,
        }
    }
}
