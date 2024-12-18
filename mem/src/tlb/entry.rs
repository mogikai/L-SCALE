use crate::physical::page::PageData;
use crate::tlb::PageTag;
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};
use std::fmt::{
    Debug,
    Formatter,
};

#[derive(Copy, Clone)]
#[repr(C)]
#[repr(align(16))]
pub struct TLBEntry {
    pub tag: u64,
    /// The target host address of the page. Since an HVAddr can never be null (see [NonNull]),
    /// the Option<HVAddr> type has the same size as a native pointer, and as such we can use it in
    /// the jit code like a normal address.
    pub target: Option<HVAddr<PageData>>,
}

impl Debug for TLBEntry {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        if self.tag == u64::MAX {
            write!(f, "E")
        } else {
            write!(f, "{:X}->{:?}", self.tag, self.target.unwrap())
        }
    }
}

impl Default for TLBEntry {
    fn default() -> Self {
        Self {
            tag: u64::MAX,
            target: None,
        }
    }
}

impl TLBEntry {
    pub fn new(
        addr: EVAddr,
        target: HVAddr<PageData>,
    ) -> Self {
        Self {
            tag: addr.tag(),
            target: Some(target),
        }
    }
}
