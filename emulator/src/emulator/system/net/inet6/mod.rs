pub mod tcp;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::handles::HandleOps;
use crate::emulator::system::net::SocketType;
use byteorder::ByteOrder;

pub fn from_type(typ: SocketType) -> Result<Box<dyn HandleOps>, GenericError> {
    Ok(match typ {
        SocketType::SOCK_STREAM => Box::new(tcp::TcpSock::new()),
        _ => todo!(),
    })
}
