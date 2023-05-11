use byteorder::ByteOrder;
use crate::emulator::abstractions::mmu::encoding::{Encode, RequiredMem};

impl RequiredMem for i64 {}

impl<Endianness: ByteOrder> Encode<Endianness> for i64 {
    fn encode(&self) -> Vec<u8> {
        let mut data = vec![0; Encode::<Endianness>::encoded_size(self)];
        Endianness::write_i64(&mut data[..8], *self);
        data
    }
}

impl RequiredMem for &[u8] {
    fn required_mem() -> usize {
        std::mem::size_of::<u8>()
    }
}

impl<Endianness: ByteOrder> Encode<Endianness> for &[u8] {
    fn encode(&self) -> Vec<u8> {
        self.to_vec()
    }

    fn encoded_size(&self) -> usize {
        self.len() * Self::required_mem()
    }
}