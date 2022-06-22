use core::ops::Range;
use std::cell::{Ref, RefCell, RefMut};
use std::cmp::{max, Ordering};
use std::cmp::min;
use std::collections::Bound::Excluded;
use std::collections::BTreeSet;
use std::ffi::CString;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use byteorder::{ByteOrder, LittleEndian};

use crate::emulator::abstractions::cpu::{CPU, Permission, Vaddr};

const MASK4K: u64 = 4 * (1 << 10) - 1;

pub struct Memory {
    mappings: BTreeSet<Region>,
    heap: u64,
    _stack: u64,
    mapbase: u64,
}

fn align4k_up(address: u64) -> u64 {
    return (address + MASK4K) & !MASK4K;
}

fn align4k_down(address: u64) -> u64 {
    return address & !MASK4K;
}

fn is4kaligned(address: u64) -> bool {
    return (address & MASK4K) == 0;
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
    fn writeat(&mut self, address: u64, data: T) -> u64;
    fn writeto(&mut self, address: u64, data: T) -> u64;
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

impl MemoryWrite<u64> for Memory {
    fn writeat(&mut self, address: u64, data: u64) -> u64 {
        let mut buf: Vec<u8> = vec![0; 8];
        LittleEndian::write_u64(&mut buf, data);
        return self.writeat(address, &buf);
    }
    fn writeto(&mut self, address: u64, data: u64) -> u64 {
        let mut buf: Vec<u8> = vec![0; 8];
        LittleEndian::write_u64(&mut buf, data);
        return self.writeto(address, &buf);
    }
}

impl MemoryWrite<&[u8]> for Memory {
    fn writeat(&mut self, address: u64, data: &[u8]) -> u64 {
        self.write_intern(address, data);
        return data.len().try_into().unwrap();
    }
    fn writeto(&mut self, address: u64, data: &[u8]) -> u64 {
        let len = data.len().try_into().unwrap();
        self.write_intern(address - len, data);
        return len;
    }
}

impl MemoryWrite<&[(u64, u64)]> for Memory {
    fn writeat(&mut self, address: u64, data: &[(u64, u64)]) -> u64 {
        let d: Vec<u64> = data.iter().fold(Vec::new(), |mut v, (a, b)| {
            v.push(*a);
            v.push(*b);
            v
        });
        return self.writeat(address, &d);
    }

    fn writeto(&mut self, address: u64, data: &[(u64, u64)]) -> u64 {
        let d: Vec<u64> = data.iter().fold(Vec::new(), |mut v, (a, b)| {
            v.push(*a);
            v.push(*b);
            v
        });
        return self.writeto(address, &d);
    }
}

impl MemoryWrite<&Vec<(u64, u64)>> for Memory {
    fn writeat(&mut self, address: u64, data: &Vec<(u64, u64)>) -> u64 {
        return self.writeat(address, data.as_slice());
    }
    fn writeto(&mut self, address: u64, data: &Vec<(u64, u64)>) -> u64 {
        return self.writeto(address, data.as_slice());
    }
}

impl MemoryWrite<&[u64]> for Memory {
    fn writeat(&mut self, address: u64, data: &[u64]) -> u64 {
        let mut d: Vec<u8> = Vec::with_capacity(data.len() * 8);
        LittleEndian::write_u64_into(&data, &mut d);
        self.write_intern(address, &d);
        return d.len().try_into().unwrap();
    }
    fn writeto(&mut self, address: u64, data: &[u64]) -> u64 {
        let mut d: Vec<u8> = vec!(0; data.len() * 8);
        LittleEndian::write_u64_into(&data, &mut d);
        let len = d.len().try_into().unwrap();
        self.write_intern(address - len, &d);
        return len;
    }
}

impl MemoryWrite<&Vec<u64>> for Memory {
    fn writeat(&mut self, address: u64, data: &Vec<u64>) -> u64 {
        return self.writeat(address, data.as_slice());
    }
    fn writeto(&mut self, address: u64, data: &Vec<u64>) -> u64 {
        return self.writeto(address, data.as_slice());
    }
}

impl MemoryWrite<&Vec<u8>> for Memory {
    fn writeat(&mut self, address: u64, data: &Vec<u8>) -> u64 {
        return self.writeat(address, data.as_slice());
    }
    fn writeto(&mut self, address: u64, data: &Vec<u8>) -> u64 {
        return self.writeto(address, data.as_slice());
    }
}

impl MemoryWrite<Ref<'_, [u8]>> for Memory {
    fn writeat(&mut self, address: u64, data: Ref<'_, [u8]>) -> u64 {
        let d: &[u8] = data.deref();
        return self.writeat(address, d);
    }
    fn writeto(&mut self, address: u64, data: Ref<'_, [u8]>) -> u64 {
        let d: &[u8] = data.deref();
        return self.writeto(address, d);
    }
}

impl MemoryWrite<&str> for Memory {
    fn writeat(&mut self, address: u64, data: &str) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        return self.writeat(address, d.as_bytes_with_nul());
    }
    fn writeto(&mut self, address: u64, data: &str) -> u64 {
        let v: Vec<u8> = data.as_bytes().to_vec();
        let d: CString = unsafe { CString::from_vec_unchecked(v) };
        return self.writeto(address, d.as_bytes_with_nul());
    }
}

#[derive(Debug)]
pub enum ReadError {
    NotMapped,
}

impl<'a> Memory {
    pub fn new(stackbase: u64, heapbase: u64) -> Memory {
        let map: BTreeSet<Region> = BTreeSet::new();
        return Memory { mappings: map, heap: heapbase, _stack: stackbase, mapbase: 0x100000 };
    }

    pub fn read_intern(&self, address: u64, size: u64) -> Result<Ref<[u8]>, ReadError> {
        let region = self.get_region(address).ok_or(ReadError::NotMapped)?;
        let d = Ref::map(region.as_slice(), |d| &d[(address - region.vaddr).try_into().unwrap()..]);
        let size = min(d.len(), size.try_into().unwrap());
        return Ok(Ref::map(d, |d| &d[..size]));
    }

    fn write_intern(&self, address: u64, data: &[u8]) {
        let region = self.get_region(address).unwrap();
        let offset = (address - region.vaddr).try_into().unwrap();
        let r = Range { start: offset, end: offset + data.len() };
        let mut memory: RefMut<'_, [u8]> = RefMut::map(region.as_mut_slice(), |d| &mut d[r]);
        memory.copy_from_slice(data);
    }

    pub fn map_data<'b, CpuT: CPU<'b>>(&mut self, data: &[u8], vm_range: Range<u64>, perms: Permission, cpu: &mut CpuT) -> Result<(), MappingError> {
        let start = align4k_down(vm_range.start);
        let size = align4k_up(vm_range.end) - start;
        if self.get_region(start).is_some() || self.get_region(start + size).is_some() {
            panic!("not implemented");
        }
        self.fixed_mapping(start, size, perms, cpu)?;
        self.write_intern(vm_range.start, data);
        return Ok(());
    }

    pub fn mmap<'b, CpuT: CPU<'b>>(&mut self, address: u64, size: u64, perms: Permission, cpu: &mut CpuT) -> Result<u64, MappingError> {
        let size = align4k_up(size);
        if !is4kaligned(address) {
            return Err(MappingError::NotAligned);
        }
        if size == 0 {
            return Err(MappingError::Zero);
        }
        if address == 0 {
            return self.new_mapping(size, perms, cpu);
        } else if self.get_region(address).is_some() || self.get_region(address + size - 1).is_some() {
            return Err(MappingError::AlreadyMapped);
        }
        return self.fixed_mapping(address, size, perms, cpu);
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
            Some(r) => self.heap + r.size,
            None => self.heap,
        }
    }

    fn new_mapping<'b, CpuT: CPU<'b>>(&mut self, size: u64, perms: Permission, cpu: &mut CpuT) -> Result<u64, MappingError> {
        let address = self.find_free(0, size);
        self.fixed_mapping(address, size, perms, cpu)?;
        return Ok(address);
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
        let size = region.size.try_into().unwrap();
        if region.offset + region.size > mem.len().try_into().unwrap() {
            panic!("bla");
        }

        unsafe { cpu.mem_map_ptr(address, size, region.perms, ptr as _).expect("Mapping error"); };
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
            let regions = region.cutout(*vaddr, address, size);
            self.mappings.extend(regions);
        }
        Ok(())
    }

    fn unmap_intern<'b, CpuT: CPU<'b>>(&self, vaddr: Vaddr, size: u64, cpu: &mut CpuT) {
        info!("unmap: vaddr: 0x{:X}, size: 0x{:X}", vaddr, size);
        cpu.mem_unmap(vaddr, size.try_into().unwrap()).expect("Failed to unmap");
    }

    pub fn find_free(&self, hint: Vaddr, size: u64) -> u64 {
        // let mut hint = max(hint, self.mapbase);
        // for region in self.mappings.range(hint..) {
        //     if region.contains(hint) || region.contains(hint + size) {
        //         hint = region.vaddr + region.size;
        //     } else {
        //         break;
        //     }
        // }
        // return hint;
        let mut h: u64 = max(hint, self.mapbase);
        for region in self.mappings.iter() {
            if h > region.vaddr {
                continue;
            }
            if h + size >= region.vaddr {
                h = region.vaddr + region.size;
                continue;
            }
            break;
        }
        h
    }

    pub fn protect<'b, CpuT: CPU<'b>>(&mut self, start: u64, size: u64, perms: Permission, cpu: &mut CpuT) {
        let region = self.get_region(start).unwrap();
        if region.vaddr != start || size != region.size {
            if start + size > region.vaddr + region.size {
                error!("mprotect more then one region");
                panic!("not implemented yet");
            }
            let vaddr = region.vaddr;
            let region = self.mappings.take(&vaddr).unwrap();
            let (regions, mut r) = region.split(region.vaddr, start, size);
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
			let d = $mem.read_intern($addr, 4096).unwrap();
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
        let len = data.borrow().len();
        Region { data, offset: 0, vaddr, size: len as u64, perms }
    }

    fn contains(&self, address: Vaddr) -> bool { address >= self.vaddr && address < self.vaddr + self.size }

    fn end(&self) -> Vaddr { self.vaddr + self.size }

    fn split(&self, vaddr: Vaddr, address: u64, size: u64) -> (Vec<Region>, Region) {
        //TODO better checks
        let n = Region { data: self.data.clone(), offset: self.offset + address - vaddr, vaddr: self.vaddr, size, perms: self.perms };
        let ret = self.cutout(vaddr, address, size);
        (ret, n)
    }
    fn cutout(&self, vaddr: Vaddr, address: u64, size: u64) -> Vec<Region> {
        let mut ret: Vec<Region> = Vec::new();
        if vaddr < address {
            ret.push(Region { data: self.data.clone(), size: address - vaddr, offset: self.offset, perms: self.perms, vaddr });
        }
        if address + size < vaddr + self.size {
            ret.push(Region { data: self.data.clone(), size: vaddr + self.size - address - size, offset: self.offset + (address - vaddr) + size, perms: self.perms, vaddr: address + size });
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

