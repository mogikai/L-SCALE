use byteorder::ByteOrder;

use crate::emulator::abstractions::mmu::encoding::{Encode, RequiredMem};

impl RequiredMem for i64 {}

impl Encode for i64 {
    fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
        let start = buffer.len();
        buffer.reserve(Self::required_mem());
        unsafe { buffer.set_len(start + Self::required_mem()); }
        Endianness::write_i64(&mut buffer[start..], *self);
    }
}

impl RequiredMem for u8 {}

impl Encode for u8 {
    fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
        buffer.push(*self)
    }
}

impl RequiredMem for u16 {}

impl Encode for u16 {
    fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
        let start = buffer.len();
        buffer.reserve(Self::required_mem());
        unsafe { buffer.set_len(start + Self::required_mem()); }
        Endianness::write_u16(&mut buffer[start..], *self);
    }
}

impl RequiredMem for u32 {}

impl Encode for u32 {
    fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
        let start = buffer.len();
        buffer.reserve(Self::required_mem());
        unsafe { buffer.set_len(start + Self::required_mem()); }
        Endianness::write_u32(&mut buffer[start..], *self);
    }
}

impl RequiredMem for u64 {}

impl Encode for u64 {
    fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
        let start = buffer.len();
        buffer.reserve(Self::required_mem());
        unsafe { buffer.set_len(start + Self::required_mem()); }
        Endianness::write_u64(&mut buffer[start..], *self);
    }
}

impl RequiredMem for &[u8] {
    fn required_mem() -> usize {
        std::mem::size_of::<u8>()
    }
}

impl Encode for &[u8] {
    fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self);
    }

    fn encoded_size(&self) -> usize {
        self.len() * Self::required_mem()
    }
}