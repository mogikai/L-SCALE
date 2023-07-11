use byteorder::ByteOrder;

use crate::emulator::platform::linux::filedescriptors::FileData;
use crate::emulator::platform::linux::kernel::eventpoll::Epoll;
use crate::emulator::platform::linux::signalfd::SignalFD;
use crate::emulator::system::net::inet6::tcp::TcpSock;
use crate::emulator::system::net::unix::UnixSocketData;

#[derive(Default)]
pub struct FileDescriptor {
    pub file: Option<FileData>,
    pub signals: Option<SignalFD>,
    pub socket: Option<TcpSock>,
    pub poll: Option<Epoll>,
    pub usock: Option<UnixSocketData>,
}

impl FileDescriptor {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn new_file(
        f: FileData
    ) -> FileDescriptor {
        FileDescriptor {
            file: Some(f),
            ..Default::default()
        }
    }

    pub fn new_signalfd(
        sh: SignalFD
    ) -> FileDescriptor {
        FileDescriptor {
            signals: Some(sh),
            ..Default::default()
        }
    }

    pub fn new_socket(
        t: TcpSock,
    ) -> FileDescriptor {
        FileDescriptor {
            socket: Some(t),
            ..Default::default()
        }
    }

    pub fn new_usock(
        s: UnixSocketData,
    ) -> FileDescriptor {
        FileDescriptor {
            usock: Some(s),
            ..Default::default()
        }
    }

    pub fn new_epoll(
        e: Epoll,
    ) -> FileDescriptor {
        FileDescriptor {
            poll: Some(e),
            ..Default::default()
        }
    }
}
