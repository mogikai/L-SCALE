use byteorder::ByteOrder;
use tcp::socket;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::TCPData;
use crate::emulator::platform::linux::kernel::net::SocketType;
use crate::emulator::system::fd_map::FileDescriptor;

pub mod tcp;

pub fn from_type(typ: SocketType) -> Result<FileDescriptor<FileDescriptorData>, GenericError> {
    Ok(match typ {
        SocketType::SOCK_STREAM => {
            let mut fd = FileDescriptor::new(Box::new(socket::TcpSocketOps::new()));
            fd.data.tcp = Some(TCPData::new());
            fd
        }
        _ => todo!(),
    })
}
