use bitvec::access::BitSafeU8;
use bitvec::ptr::{
    BitRef,
    Mut,
};
use byteorder::ByteOrder;
use fuzzyemu_derive::Decode;
use tracing::trace;

use crate::emulator::abstractions::cpu::address::{
    EPAddr,
    EVAddr,
};
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::file_descriptors::FileOperationsReady;
use crate::emulator::abstractions::mmu::encoding::{
    Decode,
    Encode,
    RequiredMem,
};
use crate::emulator::abstractions::mmu::{
    MemoryError,
    MMU,
};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::FD;
use crate::emulator::platform::linux::kernel::error::SyscallError;
use crate::emulator::platform::linux::kernel::{
    sys_component_mut,
    LinuxKernelError,
    LinuxKernelTask,
    SyscallResult,
};
use crate::emulator::system::memory::rw_interface::{
    MemoryRead,
    MemoryWrite,
};
use fuzzyemu_derive::unfinished;

impl<Endianness, Arch, CpuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    #[unfinished]
    pub fn pselect6(
        &self,
        nfds: u64,
        readfds_addr: EVAddr,
        writefds_addr: EVAddr,
        exceptfds_addr: EVAddr,
        timeout_addr: EVAddr,
        sigmask_addr: EVAddr,
    ) -> SyscallResult {
        let mut memory = sys_component_mut!(self, memory);
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = memory.accessor(&mut cpu, self.task_id);
        let mut fd_map = sys_component_mut!(self, fd_map);

        let mut all_ready_fds = fd_set::new();
        let mut retain_ready_fd = |fd, mut bit: BitRef<'_, Mut, BitSafeU8>, interest| {
            let file = fd_map.get_mut(&self.task_id, &fd);
            if let Some(file) = file {
                let ready_ops = file
                    .poll(interest)
                    .expect("Only pollable fds should be in interest list");
                *bit = ready_ops.intersects(interest);
                if *bit {
                    all_ready_fds.set_fd(fd);
                }
            } else {
                *bit = false;
            }
        };

        let mut readfds: fd_set = memory.read(readfds_addr)?;
        let mut writefds: fd_set = memory.read(writefds_addr)?;
        let mut exceptfds: fd_set = memory.read(exceptfds_addr)?;
        readfds
            .iter_requested_mut()
            .take_while(|(fd, _)| fd < &nfds.into())
            .for_each(|(fd, bit)| {
                // TODO: Are these the correct events to check for?
                retain_ready_fd(
                    fd,
                    bit,
                    FileOperationsReady::Read
                        | FileOperationsReady::ReadNormal
                        | FileOperationsReady::ReadHangUp
                        | FileOperationsReady::Error,
                );
            });
        writefds
            .iter_requested_mut()
            .take_while(|(fd, _)| fd < &nfds.into())
            .for_each(|(fd, bit)| {
                // TODO: Are these the correct events to check for?
                retain_ready_fd(
                    fd,
                    bit,
                    FileOperationsReady::Write
                        | FileOperationsReady::WriteNormal
                        | FileOperationsReady::HangUp
                        | FileOperationsReady::Error,
                );
            });
        exceptfds
            .iter_requested_mut()
            .take_while(|(fd, _)| fd < &nfds.into())
            .for_each(|(fd, bit)| {
                // TODO: Are these the correct events to check for?
                retain_ready_fd(fd, bit, FileOperationsReady::ReadOutOfBand | FileOperationsReady::WriteOutOfBand);
            });
        let fds_set = all_ready_fds.number_of_fds_set();
        if fds_set == 0 {
            Err(SyscallError::Yield)?
        }
        // Only write out results after we have decided whether to yield or not.
        // If the syscall yields, it must not write anything out beforehand.
        memory.write(readfds_addr, readfds)?;
        memory.write(writefds_addr, writefds)?;
        memory.write(exceptfds_addr, exceptfds)?;
        Ok(fds_set as u64)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub(crate) struct fd_set(pub bitvec::array::BitArray<[u8; 128]>);

impl fd_set {
    /// Iterate over the requested fds.
    pub fn iter_requested_mut(&mut self) -> impl Iterator<Item = (FD, BitRef<'_, Mut, BitSafeU8>)> {
        self.0.iter_mut().enumerate().filter_map(|(index, bit)| {
            if *bit {
                Some((FD::from(index as u64), bit))
            } else {
                None
            }
        })
    }

    pub fn number_of_fds_set(&self) -> usize {
        self.0.count_ones()
    }

    pub fn set_fd(
        &mut self,
        fd: FD,
    ) {
        self.0.set(fd.into(), true);
    }

    pub fn new() -> Self {
        Self(bitvec::array::BitArray::new([0; 128]))
    }
}

impl RequiredMem for fd_set {}

impl Decode for fd_set {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        if val.len() != Self::required_mem() {
            Err(MemoryError::InsufficientSize)?
        }
        let data: [u8; 128] = val.try_into().map_err(|_| MemoryError::ConversionError)?;
        Ok(Self(bitvec::array::BitArray::from(data)))
    }
}

impl Encode for fd_set {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        buffer.extend_from_slice(self.0.as_raw_slice());
    }
}
