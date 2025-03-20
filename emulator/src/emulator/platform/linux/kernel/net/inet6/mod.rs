use byteorder::ByteOrder;
use mogi_system::components::cpu::CPU;
use mogi_system::components::mmu::MMU;
use tcp::socket;

use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorError,
    FileDescriptorOps,
};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::SocketType;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::TCPData;
use crate::emulator::platform::linux::kernel::net::inet6::udp::UDPData;
use crate::emulator::platform::linux::kernel::net::inet6::udp::socket::UdpSocketOps;
use crate::emulator::system::fd_map::FileDescriptor;
use mogi_system::errors::generic::GenericError;

pub mod tcp;
pub mod udp;

pub fn from_type<Endianness, CpuT, MmuT, TcpT>(
    typ: SocketType,
    system: &LinuxSystem<Endianness, CpuT, MmuT, TcpT>,
) -> Result<FileDescriptor<FileDescriptorData>, FileDescriptorError>
where
    Endianness: ByteOrder,
    CpuT: CPU,
    MmuT: MMU,
    TcpT: TCPSystem + 'static,
{
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
