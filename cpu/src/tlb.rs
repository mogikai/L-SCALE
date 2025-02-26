mod debug;
pub mod entry;

use crate::tlb::entry::TLBEntry;
use mogi_utilities::addresses::{
    Address,
    Addressable,
    EVAddr,
    HVAddr,
};
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::mem::offset_of;
use std::ops::BitAnd;

#[cfg(feature = "trace_tlb")]
macro_rules! trace_tlb {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*)
    };
}
#[cfg(not(feature = "trace_tlb"))]
macro_rules! trace_tlb {
    ($($arg:tt)*) => {};
}

use crate::jit::trace_jit;
pub(crate) use trace_tlb;

/// The number of bits required to represent an offset in a page from the page base address
const PAGE_OFFSET_BITS: usize = 12;

/// The size in bytes of a page
// const PAGE_SIZE: usize = 1 << PAGE_OFFSET_BITS;

// const MAX_PAGE_OFFSET: usize = (1 << PAGE_OFFSET_BITS) - 1;

// TODO: Experiment with tlb sizes, and with per-process tables
pub const TLB_INDEX_BITS: usize = 12;
const TLB_SIZE: usize = 1 << TLB_INDEX_BITS;

pub trait PageMetaInfo: Copy {
    fn page_size() -> usize;
    fn page_offset_bits() -> usize;

    fn page_offset_mask() -> u64 {
        (1 << Self::page_offset_bits()) - 1
    }

    /// Calculate the offset from the start of the page where the provided address lives.
    fn page_offset(addr: impl BitAnd<u64, Output = u64>) -> usize {
        (addr & Self::page_offset_mask()) as usize
    }

    fn page_index(addr: impl BitAnd<u64, Output = u64>) -> usize {
        (addr & Self::page_offset_mask()) as usize
    }
}

pub trait PageIndex {
    fn index<PageDataT: PageMetaInfo>(&self) -> u64;

    /// The number of bits to shift to get the index
    fn index_shift_from_start<PageDataT: PageMetaInfo>() -> u8;

    /// The mask to apply to the shifted value to extract the actual index.
    fn index_mask() -> u64;
}

pub trait PageTag {
    fn tag<PageDataT: PageMetaInfo>(&self) -> u64;

    fn tag_bits<PageDataT: PageMetaInfo>() -> u64;

    /// The number of bits to shift from the index to get the tag.
    fn tag_shift_from_index() -> u8;

    /// The mask to apply to the shifted value to extract the actual tag.
    fn tag_mask<PageDataT: PageMetaInfo>() -> u64;
}

impl<T> PageIndex for EVAddr<T> {
    fn index<PageDataT: PageMetaInfo>(&self) -> u64 {
        (self >> PageDataT::page_offset_bits()) & ((1 << TLB_INDEX_BITS) - 1)
    }

    fn index_shift_from_start<PageDataT: PageMetaInfo>() -> u8 {
        PageDataT::page_offset_bits()
            .try_into()
            .expect("Shift bits must fit in a u8")
    }

    fn index_mask() -> u64 {
        (1 << TLB_INDEX_BITS) - 1
    }
}

impl<T> PageTag for EVAddr<T> {
    fn tag<PageDataT: PageMetaInfo>(&self) -> u64 {
        (self >> (PageDataT::page_offset_bits() + TLB_INDEX_BITS)) & ((1 << Self::tag_bits::<PageDataT>()) - 1)
    }

    fn tag_bits<PageDataT: PageMetaInfo>() -> u64 {
        (Self::bits() - (PageDataT::page_offset_bits() + TLB_INDEX_BITS)) as u64
    }

    fn tag_shift_from_index() -> u8 {
        TLB_INDEX_BITS
            .try_into()
            .expect("Shift bits must fit in a u8")
    }

    fn tag_mask<PageDataT: PageMetaInfo>() -> u64 {
        (1 << Self::tag_bits::<PageDataT>()) - 1
    }
}

const PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug)]
#[repr(align(4096))]
pub struct PageData {
    data: [u8; PAGE_SIZE],
}

impl PageData {
    pub fn zero() -> Self {
        assert!(PAGE_SIZE.is_power_of_two());
        Self { data: [0; PAGE_SIZE] }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl PageMetaInfo for PageData {
    fn page_size() -> usize {
        PAGE_SIZE
    }

    fn page_offset_bits() -> usize {
        PAGE_SIZE.trailing_zeros() as usize
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TLB<PageDataT> {
    // TODO: Remove pub
    read_entries: [TLBEntry<PageDataT>; TLB_SIZE],
    write_entries: [TLBEntry<PageDataT>; TLB_SIZE],
}

#[derive(Clone, Copy, Debug)]
pub enum AccessType {
    Load,
    Store,
}

impl Display for AccessType {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AccessType::Load => "load",
                AccessType::Store => "store",
            }
        )
    }
}

impl<PageDataT> TLB<PageDataT> {
    pub fn new() -> Self {
        Self {
            read_entries: [Default::default(); TLB_SIZE],
            write_entries: [Default::default(); TLB_SIZE],
        }
    }

    pub fn flush(&mut self) {
        self.flush_read();
        self.flush_write();
    }

    pub fn flush_read(&mut self) {
        self.read_entries.fill_with(Default::default);
    }

    pub fn flush_write(&mut self) {
        self.write_entries.fill_with(Default::default);
    }

    pub fn read_offset() -> usize {
        offset_of!(Self, read_entries)
    }

    pub fn write_offset() -> usize {
        offset_of!(Self, write_entries)
    }
}

impl<PageDataT> TLB<PageDataT>
where
    PageDataT: PageMetaInfo,
{
    pub fn insert_read(
        &mut self,
        addr: EVAddr,
        page: HVAddr<PageDataT>,
    ) {
        let entry = TLBEntry::new(addr, page);
        trace_tlb!("New read entry {:?} for {:?} at index {:#x}", entry, addr, addr.index::<PageDataT>(),);
        self.read_entries[addr.index::<PageDataT>() as usize] = entry;
    }

    pub fn insert_write(
        &mut self,
        addr: EVAddr,
        page: HVAddr<PageDataT>,
    ) {
        let entry = TLBEntry::new(addr, page);
        trace_tlb!("New write entry {:?} for {:?} at index {:#x}", entry, addr, addr.index::<PageDataT>(),);
        self.write_entries[addr.index::<PageDataT>() as usize] = entry;
    }

    pub fn remove(
        &mut self,
        addr: EVAddr,
    ) {
        self.read_entries[addr.index::<PageDataT>() as usize] = Default::default();
        self.write_entries[addr.index::<PageDataT>() as usize] = Default::default();
    }

    /// If the entry is contained in the tlb(r/w), the mapping is updated. Otherwise, this operation does nothing.
    pub fn update(
        &mut self,
        addr: EVAddr,
        page: HVAddr<PageDataT>,
    ) {
        let new_entry = TLBEntry::new(addr, page);
        let entry = &mut self.read_entries[addr.index::<PageDataT>() as usize];
        if entry.tag == addr.tag::<PageDataT>() {
            *entry = new_entry;
        }
        let entry = &mut self.write_entries[addr.index::<PageDataT>() as usize];
        if entry.tag == addr.tag::<PageDataT>() {
            *entry = new_entry;
        }
    }

    /// Get a pointer to the page data containing the requested address from the TLB.
    /// If the TLB contains no such entry in its read cache, None is returned.
    pub unsafe fn page(
        &self,
        addr: EVAddr,
    ) -> Option<&PageDataT> {
        let entry = self.read_entries[addr.index::<PageDataT>() as usize];
        (entry.tag == addr.tag::<PageDataT>())
            .then_some(entry.target)
            .flatten()
            .map(|addr| unsafe { addr.as_ref() })
    }

    /// Get the page data containing the requested address from the TLB.
    /// If the TLB contains no such entry, None is returned.
    pub unsafe fn page_mut(
        &self,
        addr: EVAddr,
    ) -> Option<&mut PageDataT> {
        let entry = self.write_entries[addr.index::<PageDataT>() as usize];
        (entry.tag == addr.tag::<PageDataT>())
            .then_some(entry.target)
            .flatten()
            .map(|mut addr| unsafe { addr.as_mut() })
    }
}
