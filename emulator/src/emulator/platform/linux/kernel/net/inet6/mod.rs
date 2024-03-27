use byteorder::ByteOrder;
use mogi_system::components::cpu::CPU;
use mogi_system::components::mmu::MMU;
use tcp::socket;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorError,
    FileDescriptorOps,
};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::TCPData;
use crate::emulator::platform::linux::kernel::net::inet6::udp::socket::UdpSocketOps;
use crate::emulator::platform::linux::kernel::net::inet6::udp::UDPData;
use crate::emulator::platform::linux::kernel::net::SocketType;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::fd_map::FileDescriptor;

pub mod tcp;
pub mod udp;

pub fn from_type<Endianness: ByteOrder, CpuT: CPU + MMU, TcpT: TCPSystem + 'static>(
    typ: SocketType,
    system: &LinuxSystem<Endianness, CpuT, TcpT>,
) -> Result<FileDescriptor<FileDescriptorData>, FileDescriptorError> {
    Ok(match typ {
        SocketType::SOCK_STREAM => {
            let id = system.tcp.borrow_mut().new_socket()?;
            let mut fd = FileDescriptor::new(Box::new(socket::TcpSocketOps::new()));
            fd.data.tcp = Some(TCPData::new(system.tcp.clone(), id));
            fd
        }
        SocketType::SOCK_DGRAM => {
            let mut fd = FileDescriptor::new(Box::new(UdpSocketOps::new()));
            fd.data.udp = Some(UDPData::new());
            fd
        }
        _ => todo!(),
    })
}
