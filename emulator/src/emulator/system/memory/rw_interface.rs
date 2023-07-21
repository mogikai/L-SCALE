use std::ffi::CString;

use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode};
use crate::emulator::abstractions::mmu::{MemoryError, MMU};
use crate::emulator::system::memory::accessor::MemoryAccessor;

pub trait DirectMemoryWrite<T, Endianness: ByteOrder> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError>;

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError>;
}

pub trait MemoryWrite<T: Encode, Endianness: ByteOrder> {
    fn write(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError>;

    fn write_down(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError> {
        Self::write(self, address - data.encoded_size(), data)
    }
}

pub trait MemoryRead<T: Decode, Endianness: ByteOrder> {
    fn read(
        &self,
        address: EVAddr,
    ) -> Result<T, MemoryError>;

    fn read_bounded(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<T, MemoryError>;

    fn read_n(
        &self,
        address: EVAddr,
        n: usize,
    ) -> Result<Vec<T>, MemoryError>;

    fn read_down(
        &self,
        address: EVAddr,
    ) -> Result<T, MemoryError> {
        self.read(address - T::required_mem())
    }

    fn read_down_n(
        &self,
        address: EVAddr,
        n: usize,
    ) -> Result<Vec<T>, MemoryError> {
        self.read_n(address - (n * T::required_mem()), n)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<u8, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: u8,
    ) -> Result<usize, MemoryError> {
        self.write_bytes(address, &[data])
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: u8,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - std::mem::size_of::<u8>(), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&[u8], Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError> {
        self.write_bytes(address, data)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - data.len(), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&Vec<u8>, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &Vec<u8>,
    ) -> Result<usize, MemoryError> {
        self.write_bytes(address, data.as_slice())
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &Vec<u8>,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - data.len(), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<u64, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: u64,
    ) -> Result<usize, MemoryError> {
        let mut buf = [0u8; std::mem::size_of::<u64>()];
        Endianness::write_u64(&mut buf, data);
        self.write_bytes(address, &buf)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: u64,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - std::mem::size_of::<u64>(), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&i64, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &i64,
    ) -> Result<usize, MemoryError> {
        let mut buf = [0u8; std::mem::size_of::<i64>()];
        Endianness::write_i64(&mut buf, *data);
        self.write_bytes(address, &buf)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &i64,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - std::mem::size_of::<i64>(), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&str, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &str,
    ) -> Result<usize, MemoryError> {
        self.write_direct(
            address,
            CString::new(data)
                .map_err(|_| MemoryError::ConversionError)?
                .as_bytes_with_nul(),
        )
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &str,
    ) -> Result<usize, MemoryError> {
        self.write_direct_down(
            address,
            CString::new(data)
                .map_err(|_| MemoryError::ConversionError)?
                .as_bytes_with_nul(),
        )
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&[u64], Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &[u64],
    ) -> Result<usize, MemoryError> {
        let mut buf = vec![0u8; std::mem::size_of_val(data)];
        let mut i = 0;
        Endianness::write_u64_into(data, &mut buf);
        self.write_bytes(address, &buf)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &[u64],
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - std::mem::size_of_val(data), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<Vec<u64>, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: Vec<u64>,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address, data.as_slice())
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: Vec<u64>,
    ) -> Result<usize, MemoryError> {
        self.write_direct_down(address, data.as_slice())
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&Vec<u64>, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &Vec<u64>,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address, data.as_slice())
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &Vec<u64>,
    ) -> Result<usize, MemoryError> {
        self.write_direct_down(address, data.as_slice())
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&[(u64, u64)], Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &[(u64, u64)],
    ) -> Result<usize, MemoryError> {
        let mut buf = vec![0u8; std::mem::size_of::<u64>() * 2 * data.len()];
        let mut i = 0;
        for (a, b) in data {
            Endianness::write_u64(&mut buf[i..i + std::mem::size_of::<u64>()], *a);
            i += std::mem::size_of::<u64>();
            Endianness::write_u64(&mut buf[i..i + std::mem::size_of::<u64>()], *b);
            i += std::mem::size_of::<u64>();
        }
        self.write_bytes(address, &buf)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &[(u64, u64)],
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - (data.len() * std::mem::size_of::<u64>() * 2), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<Vec<(u64, u64)>, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: Vec<(u64, u64)>,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address, data.as_slice())
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: Vec<(u64, u64)>,
    ) -> Result<usize, MemoryError> {
        self.write_direct_down(address, data.as_slice())
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&Vec<(u64, u64)>, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &Vec<(u64, u64)>,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address, data.as_slice())
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &Vec<(u64, u64)>,
    ) -> Result<usize, MemoryError> {
        self.write_direct_down(address, data.as_slice())
    }
}

impl<Endianness: ByteOrder, MmuT: MMU, T: Decode> MemoryRead<T, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn read(
        &self,
        address: EVAddr,
    ) -> Result<T, MemoryError> {
        T::decode::<Endianness>(self.read_bytes(address, T::required_mem())?.as_slice())
    }

    fn read_bounded(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<T, MemoryError> {
        T::decode::<Endianness>(self.read_bytes(address, size)?.as_slice())
    }

    fn read_n(
        &self,
        address: EVAddr,
        n: usize,
    ) -> Result<Vec<T>, MemoryError> {
        self.read_bytes(address, T::required_mem() * n)?
            .chunks(T::required_mem())
            .map(|chunk| try { T::decode::<Endianness>(chunk)? })
            .collect()
    }
}

impl<Endianness: ByteOrder, MmuT: MMU, T: Encode> MemoryWrite<T, Endianness>
    for MemoryAccessor<'_, Endianness, MmuT>
{
    fn write(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> Result<usize, MemoryError> {
        let mut buffer = Vec::with_capacity(data.encoded_size());
        data.encode::<Endianness>(&mut buffer);
        self.write_bytes(address, buffer.as_slice())
    }
}
