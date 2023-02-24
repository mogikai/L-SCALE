mod tcp;

use byteorder::ByteOrder;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::system::net::SocketType;

pub
fn from_type<Endianness: ByteOrder>(typ: SocketType) -> Result<Box<dyn Handle<Endianness>>, GenericError> {
    Ok(match typ {
        SocketType::SOCK_STREAM => {
            Box::new(tcp::TcpSock::new())
        }
        _ => todo!(),
    })
}