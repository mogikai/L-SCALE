use byteorder::ByteOrder;
use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::mmu::{DirectMemoryWrite, MemoryError, MMU};
use crate::emulator::abstractions::mmu::accessor::MemoryAccessor;

impl<'a, Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<u8, Endianness> for MemoryAccessor<'a, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: u8,
    ) -> Result<usize, MemoryError> {
        self.mmu.mem_write_bytes(address, &[data])
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
        self.mmu.mem_write_bytes(address, data)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - data.len(), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&Vec<u8>, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &Vec<u8>,
    ) -> Result<usize, MemoryError> {
        self.mmu.mem_write_bytes(address, data.as_slice())
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
        self.mmu.mem_write_bytes(address, &buf)
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
        self.mmu.mem_write_bytes(address, &buf)
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
        self.mmu.mem_write_bytes(address, data.as_bytes())
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &str,
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - data.len(), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&[u64], Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &[u64],
    ) -> Result<usize, MemoryError> {
        let mut buf = vec![0u8; std::mem::size_of::<u64>() * data.len()];
        let mut i = 0;
        for val in data {
            Endianness::write_u64(&mut buf[i..i+std::mem::size_of::<u64>()], *val);
            i += std::mem::size_of::<u64>();
        }
        self.mmu.mem_write_bytes(address, &buf)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &[u64],
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - (data.len() * std::mem::size_of::<u64>()), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<Vec<u64>, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
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

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&Vec<u64>, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
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

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&[(u64, u64)], Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
    fn write_direct(
        &mut self,
        address: EVAddr,
        data: &[(u64, u64)],
    ) -> Result<usize, MemoryError> {
        let mut buf = vec![0u8; std::mem::size_of::<u64>() * 2 * data.len()];
        let mut i = 0;
        for (a, b) in data {
            Endianness::write_u64(&mut buf[i..i+std::mem::size_of::<u64>()], *a);
            i += std::mem::size_of::<u64>();
            Endianness::write_u64(&mut buf[i..i+std::mem::size_of::<u64>()], *b);
            i += std::mem::size_of::<u64>();
        }
        self.mmu.mem_write_bytes(address, &buf)
    }

    fn write_direct_down(
        &mut self,
        address: EVAddr,
        data: &[(u64, u64)],
    ) -> Result<usize, MemoryError> {
        self.write_direct(address - (data.len() * std::mem::size_of::<u64>() * 2), data)
    }
}

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<Vec<(u64, u64)>, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
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

impl<Endianness: ByteOrder, MmuT: MMU> DirectMemoryWrite<&Vec<(u64, u64)>, Endianness> for MemoryAccessor<'_, Endianness, MmuT> {
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