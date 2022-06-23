use core::ops::Range;
use std::cell::{Ref, RefCell, RefMut};
use std::cmp::{max, Ordering};
use std::cmp::min;
use std::collections::BTreeSet;
use std::ffi::CString;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use byteorder::ByteOrder;
use goblin::pe::import::Bitfield;

use crate::emulator::abstractions::cpu::{CPU, Permission, Vaddr};

const MASK4K: u64 = 4 * (1 << 10) - 1;

pub struct Memory<Endianness: ByteOrder> {
    mappings: BTreeSet<Region>,
    heap: Vaddr,
    _stack: Vaddr,
    mapbase: Vaddr,
    _endian: PhantomData<Endianness>
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
    NotAligned,
    Zero,
}

pub enum UnmapError {
    NotAligned,
    ZeroSize,
}

pub trait MemoryWrite<T> {
    fn writeat(&mut self, address: Vaddr, data: T) -> u64;
    fn writeto(&mut self, address: Vaddr, data: T) -> u64;
}

#[macro_export]
macro_rules! push_stack {
	($mem:ident, $data:expr, $reg:expr, $uc:ident) => {
		{
			let d = $data;
			let sp = $uc.reg_read($reg).expect("can't read stackpointer"); //TODO check
			let len = $mem.writeto(sp, d);
			$uc.reg_write($reg, sp - len).expect("can't write stackpointer");
			sp - len
		}
	}
}

impl<Endianness: ByteOrder> MemoryWrite<u64> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: u64) -> u64 {
        let mut buffer = self.access_mem_mut(address, u64::size_of()).unwrap();
        Endianness::write_u64(&mut *buffer, data);
        buffer.len() as u64
    }
    fn writeto(&mut self, address: Vaddr, data: u64) -> u64 {
        let mut buf: Vec<u8> = vec![0; 8];
        Endianness::write_u64(&mut buf, data);
        self.writeto(address, &buf)
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[u8]> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: &[u8]) -> u64 {
        self.write_internal(address, data);
        data.len() as u64
    }
    fn writeto(&mut self, address: Vaddr, data: &[u8]) -> u64 {
        let len = data.len().try_into().unwrap();
        self.write_internal(address - len, data);
        len
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[(u64, u64)]> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: &[(u64, u64)]) -> u64 {
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

    fn writeto(&mut self, address: Vaddr, data: &[(u64, u64)]) -> u64 {
        let d: Vec<u64> = data.iter().fold(Vec::new(), |mut v, (a, b)| {
            v.push(*a);
            v.push(*b);
            v
        });
        return self.writeto(address, &d);
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<(u64, u64)>> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: &Vec<(u64, u64)>) -> u64 {
        return self.writeat(address, data.as_slice());
    }
    fn writeto(&mut self, address: Vaddr, data: &Vec<(u64, u64)>) -> u64 {
        return self.writeto(address, data.as_slice());
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&[u64]> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: &[u64]) -> u64 {
        let mut buffer = self.access_mem_mut(address, data.len() * 8).unwrap();
        Endianness::write_u64_into(data, &mut *buffer);
        buffer.len() as u64
    }
    fn writeto(&mut self, address: Vaddr, data: &[u64]) -> u64 {
        let mut d: Vec<u8> = vec!(0; data.len() * 8);
        Endianness::write_u64_into(&data, &mut d);
        let len = d.len().try_into().unwrap();
        self.write_internal(address - len, &d);
        return len;
    }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<u64>> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: &Vec<u64>) -> u64 { self.writeat(address, data.as_slice()) }
    fn writeto(&mut self, address: Vaddr, data: &Vec<u64>) -> u64 { self.writeto(address, data.as_slice()) }
}

impl<Endianness: ByteOrder> MemoryWrite<&Vec<u8>> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: &Vec<u8>) -> u64 { self.writeat(address, data.as_slice()) }
    fn writeto(&mut self, address: Vaddr, data: &Vec<u8>) -> u64 { self.writeto(address, data.as_slice()) }
}

impl<Endianness: ByteOrder> MemoryWrite<Ref<'_, [u8]>> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: Ref<'_, [u8]>) -> u64 { self.writeat(address, &*data) }
    fn writeto(&mut self, address: Vaddr, data: Ref<'_, [u8]>) -> u64 { self.writeto(address, &*data) }
}

impl<Endianness: ByteOrder> MemoryWrite<&str> for Memory<Endianness> {
    fn writeat(&mut self, address: Vaddr, data: &str) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        return self.writeat(address, d.as_bytes_with_nul());
    }
    fn writeto(&mut self, address: Vaddr, data: &str) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        return self.writeto(address, d.as_bytes_with_nul());
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
            _endian: PhantomData
        }
    }

    pub fn read_internal(&self, address: u64, size: usize) -> Result<Ref<[u8]>, ReadError> {
        let region = self.get_region(address).ok_or(ReadError::NotMapped)?;
        let offset = (address - region.vaddr) as usize;
        Ok(Ref::map(region.as_slice(), |d| &d[offset..min(offset + size, region.size as usize)]))
    }

    fn access_mem_mut(&mut self, address: Vaddr, size: usize) -> Result<RefMut<[u8]>, ReadError> {
        let region = self.get_region(address).ok_or(ReadError::NotMapped)?;
        let offset = (address - region.vaddr) as usize;
        Ok(RefMut::map(region.as_mut_slice(), |slice| &mut slice[offset..offset + size]))
    }

    fn write_internal(&mut self, address: Vaddr, data: &[u8]) {
        let mut memory = self.access_mem_mut(address, data.len()).unwrap();
        memory.copy_from_slice(data);
    }

    pub fn map_data<'b, CpuT: CPU<'b>>(&mut self, data: &[u8], vm_range: Range<u64>, perms: Permission, cpu: &mut CpuT) -> Result<(), MappingError> {
        let start = align4k_down(vm_range.start);
        let size = align4k_up(vm_range.end) - start;
        if self.get_region(start).is_some() || self.get_region(start + size).is_some() {
            panic!("not implemented");
        }
        self.fixed_mapping(start, size, perms, cpu)?;
        self.write_internal(vm_range.start, data);
        Ok(())
    }

    pub fn mmap<'b, CpuT: CPU<'b>>(&mut self, address: u64, size: u64, perms: Permission, cpu: &mut CpuT) -> Result<u64, MappingError> {
        let size = align4k_up(size);
        if !is4kaligned(address) {
            return Err(MappingError::NotAligned);
        }
        if size == 0 {
            return Err(MappingError::Zero);
        }
        self.fixed_mapping(self.find_space_for_region(address, size), size, perms, cpu)
    }

    pub fn realloc_heap<'b, CpuT: CPU<'b>>(&mut self, addr: u64, cpu: &mut CpuT) {
        if !is4kaligned(addr) || addr < self.heap || addr > self.heap + 100 * 1024 * 1024 { //todo better implement max heapsize
            return;
        }
        let heap = self.get_region(self.heap);
        if heap.is_none() {
            let size = addr - self.heap;
            self.fixed_mapping(self.heap, size, Permission::READ | Permission::WRITE, cpu).unwrap();
            return;
        }
        panic!("not implemented yet");
    }

    pub fn heapend(&self) -> u64 {
        match self.get_region(self.heap) {
            Some(r) => self.heap + r.size as u64,
            None => self.heap,
        }
    }

    fn fixed_mapping<'b, CpuT: CPU<'b>>(&mut self, address: Vaddr, size: u64, perms: Permission, cpu: &mut CpuT) -> Result<u64, MappingError> {
        //TODO argscheck
        info!("fixed mapping address: 0x{:X}", address);
        let s: usize = size.try_into().unwrap();
        let v: Vec<u8> = vec![0; s];
        let data: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(v));
        let region = Region::new(data, address, perms);
        self.mappings.insert(region);
        let r = self.mappings.get(&address).unwrap();
        self.map_internal(address, r, cpu);
        Ok(address)
    }

    fn map_internal<'b, CpuT: CPU<'b>>(&self, address: u64, region: &Region, cpu: &mut CpuT) {
        let mut mem = region.data.borrow_mut(); //TODO fix offset
        let ptr: *mut u8 = mem.as_mut_ptr();
        if region.offset + region.size > mem.len() as u64 {
            panic!("bla");
        }

        unsafe { cpu.mem_map_ptr(address, region.size as usize, region.perms, ptr as _).expect("Mapping error"); };
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

    pub fn unmap<'b, CpuT: CPU<'b>>(&mut self, address: Vaddr, size: u64, cpu: &mut CpuT) -> Result<(), UnmapError> {
        if !is4kaligned(address) {
            return Err(UnmapError::NotAligned);
        }
        if size == 0 {
            return Err(UnmapError::ZeroSize);
        }
        let size = align4k_up(size);

        //TODO split regions
        //TODO unmap all
        let mut mappings: Vec<u64> = Vec::new();
        // Iterate backwards from the last known region to contain the unmapped region.
        // This way we can stop iterating once the front of the region to unmap is passed.
        for region in self.mappings.range(..address + size).rev() {
            if address < region.end() {
                let start = max(address, region.vaddr);
                let end = min(address + size, region.end());
                self.unmap_intern(start, end - start, cpu);
                mappings.push(region.vaddr);
            } else {
                break;
            }
        }
        // TODO: I believe the unmapping should be done after the cutout. Also, the cutout process is probably incorrect
        for vaddr in mappings.iter() {
            let region = self.mappings.take(vaddr).expect("Bug in memory mapping!");
            let regions = region.cutout(address, size);
            self.mappings.extend(regions);
        }
        Ok(())
    }

    fn unmap_intern<'b, CpuT: CPU<'b>>(&self, vaddr: Vaddr, size: u64, cpu: &mut CpuT) {
        info!("unmap: vaddr: 0x{:X}, size: 0x{:X}", vaddr, size);
        cpu.mem_unmap(vaddr, size.try_into().unwrap()).expect("Failed to unmap");
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

    pub fn protect<'b, CpuT: CPU<'b>>(&mut self, start: Vaddr, size: u64, perms: Permission, cpu: &mut CpuT) {
        let region = self.get_region(start).unwrap();
        if region.vaddr != start || size != region.size {
            if start + size > region.vaddr + region.size as Vaddr {
                error!("mprotect more then one region");
                panic!("not implemented yet");
            }
            let vaddr = region.vaddr;
            let region = self.mappings.take(&vaddr).unwrap();
            let (regions, mut r) = region.split(start, size);
            r.perms = perms;
            self.mappings.extend(regions);
            self.unmap_intern(start, size, cpu);
            self.map_internal(region.vaddr, &r, cpu);
            self.mappings.insert(r);
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

impl Eq for Region {}

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
    fn new(data: Rc<RefCell<Vec<u8>>>, vaddr: Vaddr, perms: Permission) -> Region {
        let size = data.borrow().len() as u64;
        Region {
            data,
            offset: 0,
            vaddr,
            size,
            perms,
        }
    }

    fn contains(&self, address: Vaddr) -> bool { address >= self.vaddr && address < self.vaddr + self.size as Vaddr }

    fn end(&self) -> Vaddr { self.vaddr + self.size as Vaddr }

    fn split(&self, address: Vaddr, size: u64) -> (Vec<Region>, Region) {
        //TODO better checks
        let n = Region {
            data: self.data.clone(),
            offset: self.offset + address - self.vaddr,
            vaddr: self.vaddr,
            size,
            perms: self.perms,
        };
        (self.cutout(address, size), n)
    }
    fn cutout(&self, address: Vaddr, size: u64) -> Vec<Region> {
        let mut ret: Vec<Region> = Vec::new();
        if self.vaddr < address {
            ret.push(Region {
                data: self.data.clone(),
                size: address - self.vaddr,
                offset: self.offset,
                perms: self.perms,
                vaddr: self.vaddr,
            });
        }
        if address + size < self.vaddr + self.size as Vaddr {
            ret.push(Region {
                data: self.data.clone(),
                size: self.vaddr + self.size - address - size,
                offset: self.offset + (address - self.vaddr) + size,
                perms: self.perms,
                vaddr: address + size,
            });
        }
        ret
    }
    fn as_slice<'a>(&'a self) -> Ref<'a, [u8]> {
        let r: Range<usize> = Range { start: self.offset.try_into().unwrap(), end: (self.offset + self.size).try_into().unwrap() };
        let d: Ref<'a, Vec<u8>> = self.data.borrow();
        Ref::map(d, |d| &d[r])
    }
    fn as_mut_slice(&self) -> RefMut<[u8]> {
        let r = Range { start: self.offset.try_into().unwrap(), end: (self.offset + self.size).try_into().unwrap() };
        let d = self.data.borrow_mut();
        RefMut::map(d, |d| &mut d[r])
    }
}

