use crate::PAGE_OFFSET_BITS;
use crate::physical::page::PageData;
use crate::tlb::entry::TLBEntry;
use crate::tlb::{
    PageIndex,
    PageTag,
    TLB_INDEX_BITS,
    TLB_SIZE,
};
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};
use std::fmt::{
    Debug,
    Formatter,
};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TLB {
    // TODO: Remove pub
    pub read_entries: [TLBEntry; TLB_SIZE],
    pub write_entries: [TLBEntry; TLB_SIZE],
}

pub enum AccessType {
    Load,
    Store,
}

impl Debug for TLB {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "TLB: {{")?;
        fn write_color(
            f: &mut Formatter<'_>,
            i: usize,
            entry: &TLBEntry,
            color: ansi_term::Colour,
        ) -> std::fmt::Result {
            if i % 10 == 0 {
                writeln!(f, "")?;
            }
            let str = if entry.target.is_some() {
                format!(
                    "{:?}->[{:?}], ",
                    EVAddr::<u8>::from(i as u64 | entry.tag << (PAGE_OFFSET_BITS + TLB_INDEX_BITS)),
                    entry
                )
            } else {
                String::from("E, ")
            };
            write!(f, "{}", color.paint(str))
        }
        for (i, entry) in self.read_entries.iter().enumerate() {
            write_color(f, i, entry, ansi_term::Colour::Green)?;
        }
        for (i, entry) in self.write_entries.iter().enumerate() {
            write_color(f, i, entry, ansi_term::Colour::Blue)?;
        }
        write!(f, "\n}}")
    }
}

impl TLB {
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

    /// If the entry is contained in the tlb(r/w), the mapping is updated. Otherwise, this operation does nothing.
    pub fn update(
        &mut self,
        addr: EVAddr,
        page: HVAddr<PageData>,
    ) {
        let new_entry = TLBEntry::new(addr, page);
        let entry = &mut self.read_entries[addr.index() as usize];
        if entry.tag == addr.tag() {
            *entry = new_entry;
        }
        let entry = &mut self.write_entries[addr.index() as usize];
        if entry.tag == addr.tag() {
            *entry = new_entry;
        }
    }

    pub fn insert_read(
        &mut self,
        addr: EVAddr,
        page: HVAddr<PageData>,
    ) {
        self.read_entries[addr.index() as usize] = TLBEntry::new(addr, page);
    }

    pub fn insert_write(
        &mut self,
        addr: EVAddr,
        page: HVAddr<PageData>,
    ) {
        self.write_entries[addr.index() as usize] = TLBEntry::new(addr, page);
    }

    pub fn remove(
        &mut self,
        addr: EVAddr,
    ) {
        self.read_entries[addr.index() as usize] = Default::default();
        self.write_entries[addr.index() as usize] = Default::default();
    }

    /// Get a pointer to the page data containing the requested address from the TLB.
    /// If the TLB contains no such entry in its read cache, None is returned.
    pub unsafe fn page(
        &self,
        addr: EVAddr,
    ) -> Option<&PageData> {
        let entry = self.read_entries[addr.index() as usize];
        (entry.tag == addr.tag())
            .then_some(entry.target)
            .flatten()
            .map(|addr| unsafe { addr.as_ref() })
    }

    /// Get the page data containing the requested address from the TLB.
    /// If the TLB contains no such entry, None is returned.
    pub unsafe fn page_mut(
        &self,
        addr: EVAddr,
    ) -> Option<&mut PageData> {
        let entry = self.write_entries[addr.index() as usize];
        (entry.tag == addr.tag())
            .then_some(entry.target)
            .flatten()
            .map(|mut addr| unsafe { addr.as_mut() })
    }
}
