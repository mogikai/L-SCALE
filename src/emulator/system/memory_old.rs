use std::cell::{Ref, RefCell, RefMut};
use std::cmp::{max, min, Ordering};
use std::collections::{btree_set, BTreeSet};
use std::ffi::CString;
use std::marker::PhantomData;
use std::ops::Range;
use std::rc::Rc;
use std::{fmt, mem};

use bitflags::bitflags;
use byteorder::ByteOrder;
use goblin::pe::import::Bitfield;

use crate::emulator::abstractions::cpu::EVAddr;

pub struct Memory<Endianness: ByteOrder> {
    stack_base: EVAddr,
    stack_size: usize,
    mappings: BTreeSet<Region>,
    heap_base: EVAddr,
    map_base: EVAddr,
    _endian: PhantomData<Endianness>,
}


pub trait MemoryWrite<T> {
    /// Writes data into memory starting at the supplied address
    /// up to the address + the length of the data in bytes.
    ///
    /// # Arguments
    ///
    /// * `address`: The address to start writing at.
    /// * `data`: The data to write into memory.
    ///
    /// returns: The size in bytes of the data that was written.
    fn write(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> u64;
    /// Writes data into memory such that the write ends at address.
    /// I.e. the write starts at the address - the length of the data in bytes.
    ///
    /// # Arguments
    ///
    /// * `address`: The address to end the write at.
    /// * `data`: The data to write into memory.
    ///
    /// returns: The size in bytes of the data that was written.
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: T,
    ) -> u64;
}


impl<Endianness: ByteOrder> MemoryWrite<u64> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: u64,
    ) -> u64 {
        let mut buffer = self.access_mem_mut(address, u64::size_of()).unwrap();
        Endianness::write_u64(&mut *buffer, data);
        buffer.len() as u64
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: u64,
    ) -> u64 {
        let mut buf: Vec<u8> = vec![0; 8];
        Endianness::write_u64(&mut buf, data);
        self.write_upto(address, &buf)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[u8]> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) -> u64 {
        self.write_internal(address, data);
        data.len() as u64
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) -> u64 {
        let len: u64 = data.len().try_into().unwrap();
        self.write(address - len, data)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[(u64, u64)]> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: &[(u64, u64)],
    ) -> u64 {
        let mut buffer = self.access_mem_mut(address, data.len() * 2 * 8).unwrap();
        let mut pos = 0;
        for (i1, i2) in data {
            Endianness::write_u64(&mut buffer[pos..pos + 8], *i1);
            pos += 8;
            Endianness::write_u64(&mut buffer[pos..pos + 8], *i2);
            pos += 8;
        }
        buffer.len() as u64
    }

    fn write_upto(
        &mut self,
        address: EVAddr,
        data: &[(u64, u64)],
    ) -> u64 {
        let len: u64 = data.len().try_into().unwrap();
        let len = len * 2 * 8;
        self.write(address - len, data)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<(u64, u64)>> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: &Vec<(u64, u64)>,
    ) -> u64 {
        self.write(address, data.as_slice())
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: &Vec<(u64, u64)>,
    ) -> u64 {
        self.write_upto(address, data.as_slice())
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[u64]> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: &[u64],
    ) -> u64 {
        let mut buffer = self.access_mem_mut(address, data.len() * 8).unwrap();
        Endianness::write_u64_into(data, &mut *buffer);
        buffer.len() as u64
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: &[u64],
    ) -> u64 {
        let len: u64 = data.len().try_into().unwrap();
        let len = len * 8;
        self.write(address - len, data)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<u64>> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: &Vec<u64>,
    ) -> u64 {
        self.write(address, data.as_slice())
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: &Vec<u64>,
    ) -> u64 {
        self.write_upto(address, data.as_slice())
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<u8>> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: &Vec<u8>,
    ) -> u64 {
        self.write(address, data.as_slice())
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: &Vec<u8>,
    ) -> u64 {
        self.write_upto(address, data.as_slice())
    }
}

impl<Endianness: ByteOrder> MemoryWrite<Ref<'_, [u8]>> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: Ref<'_, [u8]>,
    ) -> u64 {
        self.write(address, &*data)
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: Ref<'_, [u8]>,
    ) -> u64 {
        self.write_upto(address, &*data)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&str> for Memory<Endianness> {
    fn write(
        &mut self,
        address: EVAddr,
        data: &str,
    ) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        self.write(address, d.as_bytes_with_nul())
    }
    fn write_upto(
        &mut self,
        address: EVAddr,
        data: &str,
    ) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        self.write_upto(address, d.as_bytes_with_nul())
    }
}


impl<Endianness: ByteOrder> Encode<Endianness> for &[u8] {
    fn encode(&self) -> Vec<u8> { self.to_vec() }
}


impl<'a, Endianness: ByteOrder> Memory<Endianness> {
    pub fn new<'b>(
        cpu: &mut (impl MMU + Hooks<'b>),
        stack_base: EVAddr,
        stack_size: usize,
        heap_base: EVAddr,
        map_base: EVAddr,
    ) -> Memory<Endianness> {
        let mut mem = Memory {
            stack_base,
            stack_size,
            mappings: BTreeSet::new(),
            heap_base,
            map_base,
            _endian: PhantomData,
        };
        mem.map_data(
            &vec![0x41; stack_size],
            (stack_base - (stack_size as EVAddr))..stack_base,
            Permission::READ | Permission::WRITE,
            cpu,
        )
        .unwrap();
        cpu.add_unmapped_mem_hook(
            1,
            0,
            Box::new(move |_, action, addr, size, _| {
                if ((stack_base - ((stack_size + 0x10000) as EVAddr))..stack_base).contains(&addr) {
                    trace!("STACK OVERFLOW!");
                    // TODO: We want to log the stack overflow in some kind of facility such that the fuzzer can
                    // TODO: use this information in a meaningful way
                }
                trace!("{:?} on {:#X} with size {} ", action, addr, size);
                false
            }),
        );
        mem
    }

    pub fn stack_base(&self) -> EVAddr { self.stack_base }

    pub fn stack_size(&self) -> usize { self.stack_size }

    pub fn write_generic<T>(
        &self,
        addr: EVAddr,
        val: &T,
    ) -> Result<(), MemoryError>
    where
        T: Encode<Endianness>,
    {
        let data = val.encode();
        let mut start = addr;
        let end = addr + data.len() as EVAddr;
        while start < end {
            let mut buffer = self.access_mem_range_mut(start, end)?;
            let offset = (start - addr) as usize;
            let len = buffer.len();
            buffer.copy_from_slice(&data[offset..offset + len]);
            start += buffer.len() as EVAddr;
        }
        Ok(())
    }

    pub fn read_struct<T: Decode<Endianness> + RequiredMem>(
        &self,
        address: EVAddr,
    ) -> Result<T, MemoryError> {
        let mem = self.access_mem_range(address, address + T::required_mem() as EVAddr)?;
        T::decode(mem.as_ref()).map_err(|_| MemoryError::ConversionError)
    }

    /// Reads a struct but the memory supplied to decode is bounded by size
    pub fn read_struct_bounded<T: Decode<Endianness> + RequiredMem>(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<T, MemoryError> {
        let mem = self.access_mem_range(address, address + T::required_mem() as EVAddr)?;
        T::decode(&mem[..min(size, T::required_mem())]).map_err(|_| MemoryError::ConversionError)
    }

    pub fn read_structs<T: Decode<Endianness> + RequiredMem>(
        &self,
        address: EVAddr,
        count: usize,
    ) -> Result<Vec<T>, MemoryError> {
        let mut out = Vec::with_capacity(count);
        let mem = self.access_mem_range(address, address + (count * T::required_mem()) as EVAddr)?;
        for i in 0..count {
            out.push(
                T::decode(&mem[i * T::required_mem()..(i + 1) * T::required_mem()])
                    .map_err(|_| MemoryError::ConversionError)?,
            );
        }
        Ok(out)
    }

    // Copies size bytes from process memory across region boundaries.
    // Fails if less than size bytes are mapped.
    pub fn read_mem(
        &self,
        mut address: EVAddr,
        size: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        let end = address + size as EVAddr;
        let mut out = Vec::with_capacity(size);
        while address != end {
            let mem = self.access_mem_range(address, end)?;
            out.extend(mem.as_ref());
            address += mem.len() as EVAddr;
        }
        Ok(out)
    }

    /// Tries to read up to size bytes. Returns less if not everything is mapped.
    pub fn attempt_read_mem(
        &self,
        mut address: EVAddr,
        size: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        let end = address + size as EVAddr;
        let mut out = Vec::with_capacity(size);
        while address != end {
            let mem = if let Ok(data) = self.access_mem_range(address, end) {
                data
            } else {
                break;
            };
            out.extend(mem.as_ref());
            address += mem.len() as EVAddr;
        }
        Ok(out)
    }

    pub fn access_mem(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<Ref<[u8]>, MemoryError> {
        self.access_mem_range(address, address.checked_add(size as EVAddr).ok_or(MemoryError::InvalidSize)?)
    }

    pub fn access_mem_mut(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<RefMut<[u8]>, MemoryError> {
        self.access_mem_range_mut(address, address.checked_add(size as EVAddr).ok_or(MemoryError::InvalidSize)?)
    }

    pub fn access_mem_range(
        &self,
        start: EVAddr,
        end: EVAddr,
    ) -> Result<Ref<[u8]>, MemoryError> {
        if start >= end {
            Err(MemoryError::NotMapped)?
        }
        let region = self.get_region(start).ok_or(MemoryError::NotMapped)?;
        let start_offset = (start - region.vaddr) as usize;
        let end_offset = min(end - region.vaddr, region.size as EVAddr) as usize;
        Ok(Ref::map(region.as_slice(), |slice| &slice[start_offset..end_offset]))
    }

    pub fn access_mem_range_mut(
        &self,
        start: EVAddr,
        end: EVAddr,
    ) -> Result<RefMut<[u8]>, MemoryError> {
        if start > end {
            Err(MemoryError::NotMapped)?
        }
        let region = self.get_region(start).ok_or(MemoryError::NotMapped)?;
        let start_offset = (start - region.vaddr) as usize;
        let end_offset = min(end - region.vaddr, region.end() as EVAddr) as usize;
        Ok(RefMut::map(region.as_mut_slice(), |slice| &mut slice[start_offset..end_offset]))
    }

    fn write_internal(
        &mut self,
        address: EVAddr,
        data: &[u8],
    ) {
        let mut memory = self.access_mem_mut(address, data.len()).unwrap();
        // TODO: Pass memory errors outwards so that they can be handled by the emulator
        memory.copy_from_slice(data);
    }

    pub fn map_data(
        &mut self,
        data: &[u8],
        vm_range: Range<EVAddr>,
        perms: Permission,
        cpu: &mut impl MMU,
    ) -> Result<(), MemoryError> {
        let start = align4k_down(vm_range.start);
        let size = align4k_up(vm_range.end) - start;
        if data.len() > size as usize {
            todo!()
        }
        if self.get_region(start).is_some() || self.get_region(start + size).is_some() {
            todo!()
        }
        self.fixed_mapping(start, size, perms, cpu)?;
        self.write_internal(vm_range.start, data);
        Ok(())
    }

    pub fn remap(
        &mut self,
        old_addr: EVAddr,
        old_len: u64,
        new_addr: EVAddr,
        new_len: u64,
        flags: MREMAP_FLAGS,
        cpu: &mut impl MMU,
    ) -> Result<u64, MemoryError> {
        let region = self.mappings.take(&old_addr).ok_or(MemoryError::NotMapped)?;
        self.unmap_internal(&region, cpu);
        let mut region = region.resize(new_len as usize);
        if flags.contains(MREMAP_FLAGS::MREMAP_MAYMOVE) {
            region.vaddr = self.find_space_for_region(region.vaddr, region.size);
            self.map_internal(&region, cpu);
            let new_addr = region.vaddr;
            self.mappings.insert(region);
            todo!()
            // Ok(new_addr)
        } else {
            todo!("Implement remap without MAYMOVE")
        }
    }

    pub fn mmap(
        &mut self,
        address: u64,
        size: u64,
        perms: Permission,
        cpu: &mut impl MMU,
    ) -> Result<u64, MemoryError> {
        let size = align4k_up(size);
        if !is4kaligned(address) {
            return Err(MemoryError::NotAligned);
        }
        if size == 0 {
            return Err(MemoryError::ZeroSize);
        }
        self.fixed_mapping(self.find_space_for_region(address, size), size, perms, cpu)
    }

    pub fn realloc_heap(
        &mut self,
        addr: u64,
        cpu: &mut impl MMU,
    ) {
        if !is4kaligned(addr) || addr < self.heap_base || addr > self.heap_base + 100 * 1024 * 1024 {
            //todo better implement max heapsize
            return;
        }
        let heap = self.get_region(self.heap_base);
        let size = addr - self.heap_base;
        if heap.is_none() {
            self.fixed_mapping(self.heap_base, size, Permission::READ | Permission::WRITE, cpu).unwrap();
        } else {
            self.resize_region(self.heap_base, size as usize, cpu).unwrap();
        }
    }

    pub fn heapend(&self) -> u64 {
        match self.get_region(self.heap_base) {
            Some(r) => self.heap_base + r.size as u64,
            None => self.heap_base,
        }
    }

    fn fixed_mapping(
        &mut self,
        address: EVAddr,
        size: u64,
        perms: Permission,
        cpu: &mut impl MMU,
    ) -> Result<u64, MemoryError> {
        //TODO argscheck
        info!("fixed mapping address: 0x{:X}", address);
        let region = Region::new(size, address, perms);
        let region_inserted = self.mappings.insert(region);
        assert!(region_inserted);
        let r = self.mappings.get(&address).unwrap();
        self.map_internal(r, cpu);
        Ok(address)
    }

    fn map_internal(
        &self,
        region: &Region,
        cpu: &mut impl MMU,
    ) {
        let mut mem = region.data.borrow_mut();
        let ptr: *mut u8 = unsafe { mem.as_mut_ptr().offset(region.offset as isize) };
        if region.size > mem.len() as u64 {
            panic!("bla");
        }

        unsafe {
            cpu.mem_map_ptr(region.vaddr, region.size as usize, region.perms, ptr as _).expect("Mapping error");
        };
    }

    /// Gets a reference to the region containing the supplied address.
    ///
    /// # Arguments
    ///
    /// * `address`: The address that is contained in a region that shall be looked up.
    ///
    /// returns: Some(region) if the region contains the address and None otherwise.
    fn get_region(
        &self,
        address: EVAddr,
    ) -> Option<&Region> {
        self.mappings.get(&address).or_else(|| {
            self.mappings.range(..address).next_back().and_then(|region| {
                if region.contains(address) {
                    Some(region)
                } else {
                    None
                }
            })
        })
    }

    fn get_regions_range(
        &self,
        vaddr_range: Range<EVAddr>,
    ) -> Result<btree_set::Range<Region>, MemoryError> {
        let start_region = self.mappings.range(..vaddr_range.start).next_back().ok_or(MemoryError::NotMapped)?;
        Ok(self.mappings.range(start_region.vaddr..vaddr_range.end))
    }

    fn get_region_range(
        &self,
        vaddr_range: &Range<EVAddr>,
    ) -> Option<&Region> {
        self.mappings.get(&vaddr_range.start).or_else(|| {
            self.mappings.range(..vaddr_range.start).next_back().and_then(|region| {
                if region.contains(vaddr_range) {
                    Some(region)
                } else {
                    None
                }
            })
        })
    }

    fn resize_region(
        &mut self,
        address: EVAddr,
        size: usize,
        cpu: &mut impl MMU,
    ) -> Result<(), MemoryError> {
        let region = self.mappings.take(&address).ok_or(MemoryError::NotMapped)?;
        self.unmap_internal(&region, cpu);
        let region = region.resize(size);
        self.map_internal(&region, cpu);
        self.mappings.insert(region);
        Ok(())
    }

    pub fn unmap(
        &mut self,
        address: EVAddr,
        size: u64,
        cpu: &mut impl MMU,
    ) -> Result<(), MemoryError> {
        if !is4kaligned(address) {
            return Err(MemoryError::NotAligned);
        }
        if size == 0 {
            return Err(MemoryError::ZeroSize);
        }
        let size = align4k_up(size);

        //TODO split regions
        //TODO unmap all

        // Iterate backwards from the last known region to contain the unmapped region.
        // This way we can stop iterating once the front of the region to unmap is passed.
        let mappings = self
            .mappings
            .range(..address + size)
            .rev()
            .take_while(|r| address < r.end())
            .map(|r| r.vaddr)
            .collect::<Vec<_>>();

        mappings.iter().for_each(|vaddr| {
            let region = self.mappings.take(vaddr).unwrap();
            let (front, middle, back) = region.cutout(address, size);
            self.unmap_internal(&middle, cpu);
            front.map(|r| self.mappings.insert(r));
            back.map(|r| self.mappings.insert(r));
        });
        Ok(())
    }

    fn unmap_internal(
        &self,
        region: &Region,
        cpu: &mut impl MMU,
    ) {
        info!("unmap: vaddr: 0x{:X}, size: 0x{:X}", region.vaddr, region.size);
        cpu.mem_unmap(region.vaddr, region.size.try_into().unwrap()).expect("Failed to unmap");
    }

    fn update_mapping(
        &self,
        region: &Region,
        cpu: &mut impl MMU,
    ) {
        self.unmap_internal(region, cpu);
        self.map_internal(region, cpu);
    }

    pub fn find_space_for_region(
        &self,
        hint: EVAddr,
        size: u64,
    ) -> u64 {
        let mut hint = max(hint, self.map_base);
        for region in self.mappings.range(hint..) {
            if region.contains(hint) || region.contains(hint + size - 1) {
                hint = region.vaddr + region.size as EVAddr;
            } else {
                break;
            }
        }
        hint
    }

    pub fn protect(
        &mut self,
        start: EVAddr,
        size: u64,
        perms: Permission,
        cpu: &mut impl MMU,
    ) {
        let region = self.get_region(start).unwrap();
        if region.vaddr != start || size != region.size {
            if start + size > region.vaddr + region.size as EVAddr {
                error!("mprotect more then one region");
                panic!("not implemented yet");
            }
            let vaddr = region.vaddr;
            let region = self.mappings.take(&vaddr).unwrap();
            let (front, mut middle, back) = region.cutout(start, size);
            middle.perms = perms;
            front.map(|r| self.mappings.insert(r));
            back.map(|r| self.mappings.insert(r));
            self.update_mapping(&middle, cpu);
            self.mappings.insert(middle);
        }
    }
}

// TODO: Improve error handling

#[derive(Eq)]
struct Region {
    // The underlying data vector for this region. It can be shared by multiple regions that were split.
    data: Rc<RefCell<Vec<u8>>>,
    // The offset in the data vector for this region.
    offset: u64,
    // The virtual address of this memory region
    vaddr: EVAddr,
    // The size of the data in this memory region
    size: u64,
    // The permissions associated with this region.
    perms: Permission,
}

impl PartialEq for Region {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.vaddr.eq(&other.vaddr)
    }
}

impl PartialOrd for Region {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        self.vaddr.partial_cmp(&other.vaddr)
    }
}

impl Ord for Region {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.vaddr.cmp(&other.vaddr)
    }
}

impl std::borrow::Borrow<EVAddr> for Region {
    fn borrow(&self) -> &EVAddr { &self.vaddr }
}

impl fmt::Debug for Region {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        return f.debug_struct("Point").field("offset", &self.offset).field("size", &self.size).finish();
    }
}

impl Region {
    fn new(
        size: u64,
        vaddr: EVAddr,
        perms: Permission,
    ) -> Region {
        Region {
            data: Rc::new(RefCell::new(vec![0; size as usize])),
            offset: 0,
            vaddr,
            size,
            perms,
        }
    }

    fn resize(
        self,
        size: usize,
    ) -> Region {
        let mut data = self.data.borrow_mut();
        if data.len() - self.offset as usize != self.size as usize {
            todo!("Can only resize regions that are mapped to the end of a memory block");
        }
        let new_size = if (self.size as usize) < size {
            data.len() + (size - self.size as usize)
        } else {
            data.len() - (self.size as usize - size)
        };
        data.resize(new_size, b'A'); // TODO: implement traps for invalid ptr reads maybe
        drop(data);
        Self {
            size: size as u64,
            ..self
        }
    }

    fn end(&self) -> EVAddr { self.vaddr + self.size as EVAddr }

    fn cutout(
        self,
        address: EVAddr,
        size: u64,
    ) -> (Option<Region>, Region, Option<Region>) {
        if address > self.end() {
            panic!("Address out of range")
        }
        let front = if self.vaddr < address {
            Some(Region {
                data: self.data.clone(),
                size: address - self.vaddr,
                offset: self.offset,
                perms: self.perms,
                vaddr: self.vaddr,
            })
        } else {
            None
        };
        let middle = Region {
            data: self.data.clone(),
            offset: self.offset + address - self.vaddr,
            vaddr: max(address, self.vaddr), // Clamp address to front of region
            size: min(size, self.size),      // Clamp size so maximum region size
            perms: self.perms,
        };
        let back = if address + size < self.end() {
            Some(Region {
                data: self.data.clone(),
                size: self.vaddr + self.size - address - size,
                offset: self.offset + (address - self.vaddr) + size,
                perms: self.perms,
                vaddr: address + size,
            })
        } else {
            None
        };
        (front, middle, back)
    }
    fn as_slice(&self) -> Ref<[u8]> {
        let range = Range {
            start: self.offset.try_into().unwrap(),
            end: (self.offset + self.size).try_into().unwrap(),
        };
        let data = self.data.borrow();
        Ref::map(data, |d| &d[range])
    }
    fn as_mut_slice(&self) -> RefMut<[u8]> {
        let range = Range {
            start: self.offset.try_into().unwrap(),
            end: (self.offset + self.size).try_into().unwrap(),
        };
        let data = self.data.borrow_mut();
        RefMut::map(data, |d| &mut d[range])
    }
}

pub trait Contains<T> {
    fn contains(
        &self,
        val: T,
    ) -> bool;
}

impl Contains<EVAddr> for Region {
    fn contains(
        &self,
        address: EVAddr,
    ) -> bool {
        address >= self.vaddr && address < self.vaddr + self.size as EVAddr
    }
}

impl Contains<&Range<EVAddr>> for Region {
    fn contains(
        &self,
        vadd_range: &Range<EVAddr>,
    ) -> bool {
        self.contains(vadd_range.start) && self.contains(vadd_range.end)
    }
}
