use std::cell::Ref;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;

use crate::error::memory::MemoryError;
use byteorder::ByteOrder;

pub mod basic_types;

pub trait Decode: Sized + RequiredMem {
    fn decode<Endianness: ByteOrder>(val: &[u8]) -> Result<Self, MemoryError>;
}

#[derive(Default, Debug)]
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

    pub fn encode<T: Encode>(
        mut self,
        data: &T,
    ) -> Self {
        data.encode::<Endianness>(&mut self.output);
        self
    }

    pub fn encode_front<T: Encode>(
        mut self,
        data: &T,
    ) -> Self {
        let mut bytes = Vec::new();
        data.encode::<Endianness>(&mut bytes);
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

pub trait Encode: RequiredMem {
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    );
    fn encoded_size(&self) -> usize {
        Self::required_mem()
    }
}

impl<T> RequiredMem for &T where T: RequiredMem {}

impl<T> Encode for &T
where
    T: Encode,
{
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        #[allow(suspicious_double_ref_op)]
        self.deref().encode::<Endianness>(buffer)
    }
}

impl<T> RequiredMem for Ref<'_, T> where T: RequiredMem {}

impl<T> Encode for Ref<'_, T>
where
    T: Encode,
{
    fn encode<Endianness: ByteOrder>(
        &self,
        buffer: &mut Vec<u8>,
    ) {
        self.deref().encode::<Endianness>(buffer)
    }
}
