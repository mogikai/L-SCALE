use byteorder::ByteOrder;

use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};
use crate::emulator::abstractions::mmu::MemoryError;

impl RequiredMem for i32 {}

impl Encode for i32 {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        let start = buffer.len();
        buffer.resize(start + Self::required_mem(), 0);
        Endianness::write_i32(&mut buffer[start..], *self);
    }
}

impl Decode for i32 {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        if val.len() != Self::required_mem() {
            Err(MemoryError::InvalidSize)?
        }
        Ok(Endianness::read_i32(val))
    }
}

impl RequiredMem for i64 {}

impl Encode for i64 {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        let start = buffer.len();
        buffer.resize(start + Self::required_mem(), 0);
        Endianness::write_i64(&mut buffer[start..], *self);
    }
}

impl Decode for i64 {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        if val.len() != Self::required_mem() {
            Err(MemoryError::InvalidSize)?
        }
        Ok(Endianness::read_i64(val))
    }
}

impl RequiredMem for u8 {}

impl Encode for u8 {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        buffer.push(*self)
    }
}

impl Decode for u8 {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        if val.len() != Self::required_mem() {
            Err(MemoryError::InvalidSize)?
        }
        Ok(val[0])
    }
}

impl RequiredMem for u16 {}

impl Encode for u16 {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        let start = buffer.len();
        buffer.resize(start + Self::required_mem(), 0);
        Endianness::write_u16(&mut buffer[start..], *self);
    }
}

impl Decode for u16 {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        if val.len() != Self::required_mem() {
            Err(MemoryError::InvalidSize)?
        }
        Ok(Endianness::read_u16(val))
    }
}

impl RequiredMem for u32 {}

impl Encode for u32 {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        let start = buffer.len();
        buffer.resize(start + Self::required_mem(), 0);
        Endianness::write_u32(&mut buffer[start..], *self);
    }
}

impl Decode for u32 {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        if val.len() != Self::required_mem() {
            Err(MemoryError::InvalidSize)?
        }
        Ok(Endianness::read_u32(val))
    }
}

impl RequiredMem for u64 {}

impl Encode for u64 {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        let start = buffer.len();
        buffer.resize(start + Self::required_mem(), 0);
        Endianness::write_u64(&mut buffer[start..], *self);
    }
}

impl Decode for u64 {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError> {
        if val.len() != Self::required_mem() {
            Err(MemoryError::InvalidSize)?
        }
        Ok(Endianness::read_u64(val))
    }
}

impl Encode for &[u8] {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        buffer.extend_from_slice(self);
    }

    fn encoded_size(&self) -> usize {
        self.len() * u8::required_mem()
    }
}

impl<T: Encode> RequiredMem for &[T] {
    default fn required_mem() -> usize {
        unimplemented!()
    }
}

impl<T: Encode> Encode for &[T] {
    default fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        buffer.reserve(T::required_mem() * self.len());
        for x in self.iter() {
            x.encode::<Endianness>(buffer);
        }
    }

    default fn encoded_size(&self) -> usize {
        T::required_mem() * self.len()
    }
}
