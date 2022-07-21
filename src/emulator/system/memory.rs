use core::ops::Range;
use std::{fmt, mem};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp::{max, Ordering};
use std::cmp::min;
use std::collections::BTreeSet;
use std::ffi::CString;
use std::marker::PhantomData;
use std::rc::Rc;

use byteorder::ByteOrder;
use goblin::pe::import::Bitfield;

use crate::emulator::abstractions::cpu::{MMU, Permission, Vaddr};

const MASK4K: u64 = 4 * (1 << 10) - 1;

pub struct Memory<Endianness: ByteOrder> {
    mappings: BTreeSet<Region>,
    heap: Vaddr,
    _stack: Vaddr,
    mapbase: Vaddr,
    _endian: PhantomData<Endianness>,
}

fn align4k_up(address: Vaddr) -> Vaddr {
    (address + MASK4K) & !MASK4K
}

fn align4k_down(address: Vaddr) -> Vaddr {
    address & !MASK4K
}

fn is4kaligned(address: Vaddr) -> bool {
    (address & MASK4K) == 0
}

#[derive(Debug)]
pub enum MappingError {
    AlreadyMapped,
    NotMapped,
    NotAligned,
    Zero,
}

pub enum UnmapError {
    NotAligned,
    ZeroSize,
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
    fn write(&mut self, address: Vaddr, data: T) -> u64;
    /// Writes data into memory such that the write ends at address.
    /// I.e. the write starts at the address - the length of the data in bytes.
    ///
    /// # Arguments
    ///
    /// * `address`: The address to end the write at.
    /// * `data`: The data to write into memory.
    ///
    /// returns: The size in bytes of the data that was written.
    fn write_upto(&mut self, address: Vaddr, data: T) -> u64;
}

#[macro_export]
macro_rules! push_stack {
	($mem:ident, $data:expr, $reg:expr, $uc:ident) => {
		{
			let d = $data;
			let sp = $uc.reg_read($reg).expect("can't read stackpointer"); //TODO check
			let len = $mem.write_upto(sp, d);
			$uc.reg_write($reg, sp - len).expect("can't write stackpointer");
			sp - len
		}
	}
}

impl<Endianness: ByteOrder> MemoryWrite<u64> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: u64) -> u64 {
        let mut buffer = self.access_mem_mut(address, u64::size_of()).unwrap();
        Endianness::write_u64(&mut *buffer, data);
        buffer.len() as u64
    }
    fn write_upto(&mut self, address: Vaddr, data: u64) -> u64 {
        let mut buf: Vec<u8> = vec![0; 8];
        Endianness::write_u64(&mut buf, data);
        self.write_upto(address, &buf)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[u8]> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: &[u8]) -> u64 {
        self.write_internal(address, data);
        data.len() as u64
    }
    fn write_upto(&mut self, address: Vaddr, data: &[u8]) -> u64 {
        let len: u64 = data.len().try_into().unwrap();
        self.write(address - len, data)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[(u64, u64)]> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: &[(u64, u64)]) -> u64 {
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

    fn write_upto(&mut self, address: Vaddr, data: &[(u64, u64)]) -> u64 {
        let len: u64 = data.len().try_into().unwrap();
        let len = len * 2 * 8;
        self.write(address - len, data)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<(u64, u64)>> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: &Vec<(u64, u64)>) -> u64 { self.write(address, data.as_slice()) }
    fn write_upto(&mut self, address: Vaddr, data: &Vec<(u64, u64)>) -> u64 { self.write_upto(address, data.as_slice()) }
}

impl<Endianness: ByteOrder> MemoryWrite<&[u64]> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: &[u64]) -> u64 {
        let mut buffer = self.access_mem_mut(address, data.len() * 8).unwrap();
        Endianness::write_u64_into(data, &mut *buffer);
        buffer.len() as u64
    }
    fn write_upto(&mut self, address: Vaddr, data: &[u64]) -> u64 {
        let len: u64 = data.len().try_into().unwrap();
        let len = len * 8;
        self.write(address - len, data)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<u64>> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: &Vec<u64>) -> u64 { self.write(address, data.as_slice()) }
    fn write_upto(&mut self, address: Vaddr, data: &Vec<u64>) -> u64 { self.write_upto(address, data.as_slice()) }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<u8>> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: &Vec<u8>) -> u64 { self.write(address, data.as_slice()) }
    fn write_upto(&mut self, address: Vaddr, data: &Vec<u8>) -> u64 { self.write_upto(address, data.as_slice()) }
}

impl<Endianness: ByteOrder> MemoryWrite<Ref<'_, [u8]>> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: Ref<'_, [u8]>) -> u64 { self.write(address, &*data) }
    fn write_upto(&mut self, address: Vaddr, data: Ref<'_, [u8]>) -> u64 { self.write_upto(address, &*data) }
}

impl<Endianness: ByteOrder> MemoryWrite<&str> for Memory<Endianness> {
    fn write(&mut self, address: Vaddr, data: &str) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        self.write(address, d.as_bytes_with_nul())
    }
    fn write_upto(&mut self, address: Vaddr, data: &str) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        self.write_upto(address, d.as_bytes_with_nul())
    }
}

#[derive(Debug)]
pub enum ReadError {
    NotMapped,
}

impl<'a, Endianness: ByteOrder> Memory<Endianness> {
    pub fn new(stackbase: Vaddr, heapbase: Vaddr) -> Memory<Endianness> {
        Memory {
            mappings: BTreeSet::new(),
            heap: heapbase,
            _stack: stackbase,
            mapbase: 0x100000,
            _endian: PhantomData,
        }
    }

    pub fn write_struct<T>(&self, addr: Vaddr, val: T) -> Result<(), ReadError> {
        // TODO: Maybe implement differently for endianness?
        // TODO: Maybe use something like https://docs.rs/zerocopy/latest/zerocopy/trait.AsBytes.html
        // TODO: Do we want to swallow the unsafe here? This is undefined behavior in this case.
        // TODO: If we propagate this outward, then syscalls could make at least some guarantees.
        // TODO: Also, we should probably do some invalid memory access handling to find bugs in the target.
        let mut mem: RefMut<T> = unsafe { self.get_ref_mut(addr) }?;
        *mem = val;
        Ok(())
    }

    pub fn read_internal(&self, address: u64, size: usize) -> Result<Ref<[u8]>, ReadError> {
        let region = self.get_region(address).ok_or(ReadError::NotMapped)?;
        let offset = (address - region.vaddr) as usize;
        Ok(Ref::map(region.as_slice(), |d| &d[offset..min(offset + size, region.size as usize)]))
    }

    fn access_mem(&self, address: Vaddr, size: usize) -> Result<Ref<[u8]>, ReadError> {
        let region = self.get_region(address).ok_or(ReadError::NotMapped)?;
        let offset = (address - region.vaddr) as usize;
        Ok(Ref::map(region.as_slice(), |slice| &slice[offset..offset + size]))
    }

    fn access_mem_mut(&self, address: Vaddr, size: usize) -> Result<RefMut<[u8]>, ReadError> {
        let region = self.get_region(address).ok_or(ReadError::NotMapped)?;
        let offset = (address - region.vaddr) as usize;
        Ok(RefMut::map(region.as_mut_slice(), |slice| &mut slice[offset..offset + size]))
    }

    unsafe fn get_ref<T>(&self, address: Vaddr) -> Result<Ref<T>, ReadError> {
        let data = self.access_mem(address, mem::size_of::<T>())?;
        // TODO: What about endianness? Do we really have to consider it?
        Ok(Ref::map(data, |d| {
            let d: &[T] = mem::transmute(d);
            &d[0]
        }))
    }

    unsafe fn get_ref_mut<T>(&self, address: Vaddr) -> Result<RefMut<T>, ReadError> {
        let data = self.access_mem_mut(address, mem::size_of::<T>())?;
        // TODO: What about endianness? Do we really have to consider it?
        Ok(RefMut::map(data, |d| {
            let d: &mut [T] = mem::transmute(d);
            &mut d[0]
        }))
    }

    fn write_internal(&mut self, address: Vaddr, data: &[u8]) {
        let mut memory = self.access_mem_mut(address, data.len()).unwrap();
        memory.copy_from_slice(data);
    }

    pub fn map_data(&mut self, data: &[u8], vm_range: Range<u64>, perms: Permission, cpu: &mut impl MMU) -> Result<(), MappingError> {
        let start = align4k_down(vm_range.start);
        let size = align4k_up(vm_range.end) - start;
        if self.get_region(start).is_some() || self.get_region(start + size).is_some() {
            panic!("not implemented");
        }
        self.fixed_mapping(start, size, perms, cpu)?;
        self.write_internal(vm_range.start, data);
        Ok(())
    }

    pub fn mmap(&mut self, address: u64, size: u64, perms: Permission, cpu: &mut impl MMU) -> Result<u64, MappingError> {
        let size = align4k_up(size);
        if !is4kaligned(address) {
            return Err(MappingError::NotAligned);
        }
        if size == 0 {
            return Err(MappingError::Zero);
        }
        self.fixed_mapping(self.find_space_for_region(address, size), size, perms, cpu)
    }

    pub fn realloc_heap(&mut self, addr: u64, cpu: &mut impl MMU) {
        if !is4kaligned(addr) || addr < self.heap || addr > self.heap + 100 * 1024 * 1024 { //todo better implement max heapsize
            return;
        }
        let heap = self.get_region(self.heap);
        let size = addr - self.heap;
        if heap.is_none() {
            self.fixed_mapping(self.heap, size, Permission::READ | Permission::WRITE, cpu).unwrap();
        } else {
            self.resize_region(self.heap, size as usize, cpu).unwrap();
        }
    }

    pub fn heapend(&self) -> u64 {
        match self.get_region(self.heap) {
            Some(r) => self.heap + r.size as u64,
            None => self.heap,
        }
    }

    fn fixed_mapping(&mut self, address: Vaddr, size: u64, perms: Permission, cpu: &mut impl MMU) -> Result<u64, MappingError> {
        //TODO argscheck
        info!("fixed mapping address: 0x{:X}", address);
        let region = Region::new(size, address, perms);
        self.mappings.insert(region);
        let r = self.mappings.get(&address).unwrap();
        self.map_internal(r, cpu);
        Ok(address)
    }

    fn map_internal(&self, region: &Region, cpu: &mut impl MMU) {
        let mut mem = region.data.borrow_mut();
        let ptr: *mut u8 = unsafe { mem.as_mut_ptr().offset(region.offset as isize) };
        if region.size > mem.len() as u64 {
            panic!("bla");
        }

        unsafe { cpu.mem_map_ptr(region.vaddr, region.size as usize, region.perms, ptr as _).expect("Mapping error"); };
    }

    /// Gets a reference to the region containing the supplied address.
    ///
    /// # Arguments
    ///
    /// * `address`: The address that is contained in a region that shall be looked up.
    ///
    /// returns: Some(region) if the region contains the address and None otherwise.
    fn get_region(&self, address: Vaddr) -> Option<&Region> {
        self.mappings.get(&address).or_else(||
            self.mappings.range(..address).next_back().and_then(|region|
                if region.contains(address) {
                    Some(region)
                } else {
                    None
                })
        )
    }

    fn resize_region(&mut self, address: Vaddr, size: usize, cpu: &mut impl MMU) -> Result<(), MappingError> {
        let region = self.mappings.take(&address).ok_or(MappingError::NotMapped)?;
        self.unmap_intern(&region, cpu);
        let region = region.resize(size);
        self.map_internal(&region, cpu);
        self.mappings.insert(region);
        Ok(())
    }

    pub fn unmap(&mut self, address: Vaddr, size: u64, cpu: &mut impl MMU) -> Result<(), UnmapError> {
        if !is4kaligned(address) {
            return Err(UnmapError::NotAligned);
        }
        if size == 0 {
            return Err(UnmapError::ZeroSize);
        }
        let size = align4k_up(size);

        //TODO split regions
        //TODO unmap all

        // Iterate backwards from the last known region to contain the unmapped region.
        // This way we can stop iterating once the front of the region to unmap is passed.
        let mappings = self.mappings
            .range(..address + size)
            .rev()
            .take_while(|r| address < r.end())
            .map(|r| r.vaddr)
            .collect::<Vec<_>>();

        mappings.iter().for_each(|vaddr| {
            let region = self.mappings.take(vaddr).expect("Bug in memory mapping!");
            let (front, middle, back) = region.cutout(address, size);
            self.unmap_intern(&middle, cpu);
            front.map(|r| self.mappings.insert(r));
            back.map(|r| self.mappings.insert(r));
        });
        Ok(())
    }

    fn unmap_intern(&self, region: &Region, cpu: &mut impl MMU) {
        info!("unmap: vaddr: 0x{:X}, size: 0x{:X}", region.vaddr, region.size);
        cpu.mem_unmap(region.vaddr, region.size.try_into().unwrap()).expect("Failed to unmap");
    }

    fn update_mapping(&self, region: &Region, cpu: &mut impl MMU) {
        self.unmap_intern(region, cpu);
        self.map_internal(region, cpu);
    }

    pub fn find_space_for_region(&self, hint: Vaddr, size: u64) -> u64 {
        let mut hint = max(hint, self.mapbase);
        for region in self.mappings.range(hint..) {
            if region.contains(hint) || region.contains(hint + size - 1) {
                hint = region.vaddr + region.size as Vaddr;
            } else {
                break;
            }
        }
        hint
    }

    pub fn protect(&mut self, start: Vaddr, size: u64, perms: Permission, cpu: &mut impl MMU) {
        let region = self.get_region(start).unwrap();
        if region.vaddr != start || size != region.size {
            if start + size > region.vaddr + region.size as Vaddr {
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

#[macro_export]
macro_rules! read_osstring {
	($mem:expr, $addr:expr) => {
		{
            use crate::emulator::system::memory::ReadError;
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;
			//todo better define path_max
			let d = $mem.read_internal($addr, 4096).unwrap();
			//todo better check for \0 byte existens
			let s = d.split(|c| *c == 0).next().unwrap();
			let r: Result<OsString, ReadError> = Ok(OsString::from_vec(s.to_vec()));
			r
		}
	}
}

#[derive(Eq)]
struct Region {
    // The underlying data vector for this region. It can be shared by multiple regions that were split.
    data: Rc<RefCell<Vec<u8>>>,
    // The offset in the data vector for this region.
    offset: u64,
    // The virtual address of this memory region
    vaddr: Vaddr,
    // The size of the data in this memory region
    size: u64,
    // The permissions associated with this region.
    perms: Permission,
}

impl PartialEq for Region {
    fn eq(&self, other: &Self) -> bool {
        self.vaddr.eq(&other.vaddr)
    }
}

impl PartialOrd for Region {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.vaddr.partial_cmp(&other.vaddr)
    }
}

impl Ord for Region {
    fn cmp(&self, other: &Self) -> Ordering {
        self.vaddr.cmp(&other.vaddr)
    }
}

impl std::borrow::Borrow<Vaddr> for Region {
    fn borrow(&self) -> &Vaddr {
        &self.vaddr
    }
}

impl fmt::Debug for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return f.debug_struct("Point").field("offset", &self.offset).field("size", &self.size).finish();
    }
}

impl Region {
    fn new(size: u64, vaddr: Vaddr, perms: Permission) -> Region {
        Region {
            data: Rc::new(RefCell::new(vec![0; size as usize])),
            offset: 0,
            vaddr,
            size,
            perms,
        }
    }

    fn resize(self, size: usize) -> Region {
        let mut data = self.data.borrow_mut();
        if data.len() - self.offset as usize != self.size as usize {
            todo!("Can only resize regions that are mapped to the end of a memory block");
        }
        let new_size = if (self.size as usize) < size {
            data.len() + (size - self.size as usize)
        } else {
            data.len() - (self.size as usize - size)
        };
        data.resize(new_size, 0); // TODO: implement traps for invalid ptr reads maybe
        drop(data);
        Self {
            size: size as u64,
            ..self
        }
    }

    fn contains(&self, address: Vaddr) -> bool { address >= self.vaddr && address < self.vaddr + self.size as Vaddr }

    fn end(&self) -> Vaddr { self.vaddr + self.size as Vaddr }

    fn cutout(self, address: Vaddr, size: u64) -> (Option<Region>, Region, Option<Region>) {
        if address > self.end() {panic!("Address out of range")}
        let front = if self.vaddr < address {
            Some(Region {
                data: self.data.clone(),
                size: address - self.vaddr,
                offset: self.offset,
                perms: self.perms,
                vaddr: self.vaddr,
            })
        } else { None };
        let middle = Region {
            data: self.data.clone(),
            offset: self.offset + address - self.vaddr,
            vaddr: max(address, self.vaddr), // Clamp address to front of region
            size: min(size, self.size), // Clamp size so maximum region size
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
        } else { None };
        (front, middle, back)
    }
    fn as_slice(&self) -> Ref<[u8]> {
        let range = Range { start: self.offset.try_into().unwrap(), end: (self.offset + self.size).try_into().unwrap() };
        let data = self.data.borrow();
        Ref::map(data, |d| &d[range])
    }
    fn as_mut_slice(&self) -> RefMut<[u8]> {
        let range = Range { start: self.offset.try_into().unwrap(), end: (self.offset + self.size).try_into().unwrap() };
        let data = self.data.borrow_mut();
        RefMut::map(data, |d| &mut d[range])
    }
}

