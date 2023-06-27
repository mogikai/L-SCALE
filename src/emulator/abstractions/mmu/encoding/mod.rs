use std::cell::Ref;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;

use byteorder::ByteOrder;

use crate::emulator::abstractions::mmu::MemoryError;

pub mod basic_types;

pub trait Decode<Endianness: ByteOrder>: Sized + RequiredMem {
    fn decode(val: &[u8]) -> Result<Self, MemoryError>;
}

pub struct Encoder<Endianness: ByteOrder> {
    output: Vec<u8>,
    _phantom: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder> Encoder<Endianness> {
    pub fn new() -> Encoder<Endianness> {
        Encoder {
            output: vec![],
            _phantom: Default::default(),
        }
    }

    pub fn encode<T: Encode<Endianness>>(
        mut self,
        data: &T,
    ) -> Self {
        self.output.extend(data.encode());
        self
    }

    pub fn encode_front<T: Encode<Endianness>>(
        mut self,
        data: &T,
    ) -> Self {
        let mut bytes = data.encode();
        mem::swap(&mut bytes, &mut self.output);
        self.output.extend(bytes);
        self
    }

    pub fn encode_with<F>(
        mut self,
        encode: F,
    ) -> Self
    where
        F: Fn(Self) -> Self,
    {
        encode(self)
    }

    pub fn finalize(self) -> Vec<u8> {
        self.output
    }

    pub fn encoded_length(&self) -> usize {
        self.output.len()
    }
}

pub trait RequiredMem: Sized {
    fn required_mem() -> usize {
        mem::size_of::<Self>()
    }
}

pub trait Encode<Endianness: ByteOrder>: RequiredMem {
    fn encode(&self) -> Vec<u8>;
    fn encoded_size(&self) -> usize {
        Self::required_mem()
    }
}

impl<T> RequiredMem for &T where T: RequiredMem {}

impl<T, Endianness: ByteOrder> Encode<Endianness> for &T
where
    T: Encode<Endianness>,
{
    fn encode(&self) -> Vec<u8> {
        #[allow(suspicious_double_ref_op)]
        self.deref().encode()
    }
}

impl<T> RequiredMem for Ref<'_, T> where T: RequiredMem {}

impl<T, Endianness: ByteOrder> Encode<Endianness> for Ref<'_, T>
where
    T: Encode<Endianness>,
{
    fn encode(&self) -> Vec<u8> {
        self.deref().encode()
    }
}
