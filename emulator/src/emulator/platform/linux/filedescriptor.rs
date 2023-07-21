use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::kernel::eventpoll::EpollData;
use crate::emulator::platform::linux::kernel::fs::FileData;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::TcpSock;
use crate::emulator::platform::linux::kernel::net::unix::UnixSocketData;
use crate::emulator::platform::linux::signalfd::SignalFD;

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

// TODO: Move to platform directory and make FileDescriptor generic
#[derive(Default, Debug)]
pub struct FileDescriptorData {
    pub file: Option<FileData>,
    pub signals: Option<SignalFD>,
    pub socket: Option<TcpSock>,
    pub epoll: Option<EpollData>,
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
