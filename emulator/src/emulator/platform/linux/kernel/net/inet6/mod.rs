use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::SocketType;

pub mod tcp;

pub fn from_type(typ: SocketType) -> Result<Box<dyn FileDescriptorOps<FileDescriptorData>>, GenericError> {
    Ok(match typ {
        SocketType::SOCK_STREAM => Box::new(tcp::TcpSock::new()),
        _ => todo!(),
    })
}
