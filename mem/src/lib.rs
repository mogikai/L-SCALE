use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};

pub mod deprecated;
pub mod mmu;
pub mod page_table;
pub mod physical;
pub mod tlb;

/// The number of bits required to represent an offset in a page from the page base address
pub const PAGE_OFFSET_BITS: usize = 12;

/// The size in bytes of a page
pub const PAGE_SIZE: usize = 1 << PAGE_OFFSET_BITS;

pub const MAX_PAGE_OFFSET: usize = (1 << PAGE_OFFSET_BITS) - 1;

pub trait PageOffset {
    fn offset(&self) -> usize;
}

impl<T> PageOffset for EVAddr<T> {
    fn offset(&self) -> usize {
        (*self & ((1 << PAGE_OFFSET_BITS) - 1)) as usize
    }
}

impl<T> PageOffset for EPAddr<T> {
    fn offset(&self) -> usize {
        (*self & ((1 << PAGE_OFFSET_BITS) - 1)) as usize
    }
}

////////////////
// Deprecated //
////////////////
//
// /// The value used to fill uninitalized memory
// pub const UNINIT_VALUE: u8 = 0xaa;
//
// #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub struct Addr {
//     /// The virtual address of the instruction, used for determining the next instruction and
//     /// setting the instruction pointer.
//     pub virt: u64,
//
//     /// The physical address of the instruction, used for managing the instruction cache.
//     pub phys: deprecated::physical::PhysicalAddr,
// }
//
// impl std::fmt::Display for Addr {
//     fn fmt(
//         &self,
//         f: &mut std::fmt::Formatter<'_>,
//     ) -> std::fmt::Result {
//         write!(f, "0x{:016x} ({})", self.virt, self.phys)
//     }
// }
//
// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// pub struct PhysicalMapping {
//     /// The starting address to prevent two distinct virtual mappings to the same physical address
//     /// from being merged.
//     // @todo: consider specializing the RangeMap data structure to avoid the need for this field.
//     pub addr: u64,
//
//     /// The physical index of this page associated with the mapping.
//     pub index: deprecated::physical::PageIndex,
// }
//
// #[derive(Clone, PartialEq, Eq)]
// pub enum MemoryMapping {
//     /// Represents a region of memory backed by a physical page.
//     Physical(PhysicalMapping),
//
//     /// Represents a region of memory where all data is stored inline.
//     Unallocated(UnallocatedMemory),
//
//     /// Represents a region of memory handled externally.
//     Io(usize),
// }
//
// impl std::fmt::Debug for MemoryMapping {
//     fn fmt(
//         &self,
//         f: &mut std::fmt::Formatter<'_>,
//     ) -> std::fmt::Result {
//         match self {
//             Self::Physical(inner) => write!(f, "{:?}", inner.index),
//             Self::Unallocated(inner) => write!(f, "{}", inner),
//             Self::Io(i) => write!(f, "io[{}]", i),
//         }
//     }
// }
//
// /// Used for regions of memory that need custom behaviour for every read/write.
// pub trait IoMemory {
//     fn read(
//         &mut self,
//         addr: u64,
//         buf: &mut [u8],
//     ) -> MemResult<()>;
//     fn write(
//         &mut self,
//         addr: u64,
//         value: &[u8],
//     ) -> MemResult<()>;
//
//     fn snapshot(&mut self) -> Box<dyn Any> {
//         Box::new(())
//     }
//
//     fn restore(
//         &mut self,
//         snapshot: &Box<dyn Any>,
//     ) {
//         let _ = snapshot;
//     }
// }
//
// pub trait IoMemoryAny: IoMemory {
//     fn as_any(&self) -> &dyn Any;
//     fn as_mut_any(&mut self) -> &mut dyn Any;
// }
//
// impl<T: IoMemory + 'static> IoMemoryAny for T {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }
//
//     fn as_mut_any(&mut self) -> &mut dyn Any {
//         self
//     }
// }
//
// pub struct NullMemory;
//
// impl IoMemory for NullMemory {
//     fn read(
//         &mut self,
//         _addr: u64,
//         _buf: &mut [u8],
//     ) -> MemResult<()> {
//         Ok(())
//     }
//
//     fn write(
//         &mut self,
//         _addr: u64,
//         _value: &[u8],
//     ) -> MemResult<()> {
//         Ok(())
//     }
// }
//
// #[derive(Debug, Copy, Clone)]
// pub struct IoHandler(usize);
//
// impl From<IoHandler> for MemoryMapping {
//     fn from(value: IoHandler) -> Self {
//         MemoryMapping::Io(value.0)
//     }
// }
//
// pub type Mapping = UnallocatedMemory;
//
// /// Represents a region of memory that has no physical backing, and does not need to be page
// /// aligned.
// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
// pub struct UnallocatedMemory {
//     pub perm: u8,
//     pub value: u8,
// }
//
// impl UnallocatedMemory {
//     /// Gets whether this region of memory could be replaced with the zero page for reads.
//     pub fn is_zero(self) -> bool {
//         self.value == 0x00 && perm::check(self.perm | perm::MAP, perm::READ | perm::INIT).is_ok()
//     }
// }
//
// impl std::fmt::Display for UnallocatedMemory {
//     fn fmt(
//         &self,
//         f: &mut std::fmt::Formatter<'_>,
//     ) -> std::fmt::Result {
//         write!(
//             f,
//             "Unallocated {{ perm: {} ({:#0b}), value: {:#0x} }}",
//             perm::display(self.perm),
//             self.perm,
//             self.value
//         )
//     }
// }
//
// impl From<UnallocatedMemory> for MemoryMapping {
//     fn from(v: UnallocatedMemory) -> Self {
//         Self::Unallocated(v)
//     }
// }
//
// pub type Snapshot = std::sync::Arc<SnapshotData>;
//
// pub type VirtualMemoryMap = RangeMap<MemoryMapping>;
//
// pub struct SnapshotData {
//     /// The virtual address mapping of the snapshot.
//     pub mapping: VirtualMemoryMap,
//
//     /// A snapshot of the physical memory state.
//     pub physical: deprecated::physical::PhysicalMemory,
//
//     /// The parent of this snapshot.
//     pub parent: Option<Snapshot>,
//
//     /// The snapshot state of all peripherals.
//     // @todo: need to handle dynamic adding of I/O handlers.
//     pub io: Vec<Box<dyn Any>>,
// }
//
// impl SnapshotData {
//     pub fn new() -> Self {
//         Self {
//             mapping: VirtualMemoryMap::new(),
//             physical: deprecated::physical::PhysicalMemory::new(0),
//             parent: None,
//             io: vec![],
//         }
//     }
// }
//
// impl Default for SnapshotData {
//     fn default() -> Self {
//         Self::new()
//     }
// }
//
// #[derive(Copy, Clone, Default, Debug)]
// pub struct AllocLayout {
//     /// The preferred address of the allocation
//     pub addr: Option<u64>,
//
//     /// The size of the allocation
//     pub size: u64,
//
//     /// The required alignment of the allocation
//     pub align: u64,
// }
//
// impl AllocLayout {
//     pub const fn from_size_align(
//         size: u64,
//         align: u64,
//     ) -> Self {
//         Self {
//             addr: None,
//             size,
//             align,
//         }
//     }
// }
