use bitflags::{bitflags, Flags};
use byteorder::ByteOrder;

use fuzzyemu_derive::RequiredMem;

use crate::emulator::abstractions::file_descriptors::FileOperationsReady;
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode};
use crate::emulator::abstractions::mmu::MemoryError;

bitflags! {
    #[derive(Debug, RequiredMem, Copy, Clone)]
    #[repr(C)]
    pub struct EPOLL_EVENTS: u32 {
        const EPOLLIN = 0x001;
        const EPOLLPRI = 0x002;
        const EPOLLOUT = 0x004;
        const EPOLLRDNORM = 0x040;
        const EPOLLRDBAND = 0x080;
        const EPOLLWRNORM = 0x100;
        const EPOLLWRBAND = 0x200;
        const EPOLLMSG = 0x400;
        const EPOLLERR = 0x008;
        const EPOLLHUP = 0x010;
        const EPOLLRDHUP = 0x2000;
        const EPOLLEXCLUSIVE = 1 << 28;
        const EPOLLWAKEUP = 1 << 29;
        const EPOLLONESHOT = 1 << 30;
        const EPOLLET = 1 << 31;
    }
}

impl Decode for EPOLL_EVENTS {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        Self::from_bits(Endianness::read_u32(val)).ok_or(MemoryError::ConversionError)
    }
}

impl Encode for EPOLL_EVENTS {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        self.bits().encode::<Endianness>(buffer)
    }
}

impl From<FileOperationsReady> for EPOLL_EVENTS {
    fn from(val: FileOperationsReady) -> Self {
        let mut out = EPOLL_EVENTS::empty();
        if val.contains(FileOperationsReady::Read) {
            out |= EPOLL_EVENTS::EPOLLIN;
        }
        if val.contains(FileOperationsReady::Priority) {
            out |= EPOLL_EVENTS::EPOLLPRI;
        }
        if val.contains(FileOperationsReady::Write) {
            out |= EPOLL_EVENTS::EPOLLOUT;
        }
        if val.contains(FileOperationsReady::Error) {
            out |= EPOLL_EVENTS::EPOLLERR;
        }
        if val.contains(FileOperationsReady::HangUp) {
            out |= EPOLL_EVENTS::EPOLLHUP;
        }
        if val.contains(FileOperationsReady::ReadNormal) {
            out |= EPOLL_EVENTS::EPOLLRDNORM;
        }
        if val.contains(FileOperationsReady::ReadOutOfBand) {
            out |= EPOLL_EVENTS::EPOLLRDBAND;
        }
        if val.contains(FileOperationsReady::WriteNormal) {
            out |= EPOLL_EVENTS::EPOLLWRNORM;
        }
        if val.contains(FileOperationsReady::WriteOutOfBand) {
            out |= EPOLL_EVENTS::EPOLLWRBAND;
        }
        if val.contains(FileOperationsReady::ReadHangUp) {
            out |= EPOLL_EVENTS::EPOLLRDHUP;
        }
        out
    }
}
