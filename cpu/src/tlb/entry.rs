use crate::tlb::{
    PageMetaInfo,
    PageTag,
};
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};
use std::fmt::{
    Debug,
    Formatter,
};
use std::mem::offset_of;

#[repr(C)]
#[repr(align(16))]
pub struct TLBEntry<PageDataT> {
    pub tag: u64,
    /// The target host address of the page. Since an HVAddr can never be null (see [NonNull]),
    /// the Option<HVAddr> type has the same size as a native pointer, and as such we can use it in
    /// the jit code like a normal address.
    pub target: Option<HVAddr<PageDataT>>,
}

impl<PageDataT> Copy for TLBEntry<PageDataT> {}
impl<PageDataT> Clone for TLBEntry<PageDataT> {
    fn clone(&self) -> Self {
        Self {
            tag: self.tag,
            target: self.target,
        }
    }
}

impl<PageDataT> Debug for TLBEntry<PageDataT> {
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

impl<PageDataT> Default for TLBEntry<PageDataT> {
    fn default() -> Self {
        Self {
            tag: u64::MAX,
            target: None,
        }
    }
}

impl<PageDataT> TLBEntry<PageDataT>
where
    PageDataT: PageMetaInfo,
{
    pub fn new(
        addr: EVAddr,
        target: HVAddr<PageDataT>,
    ) -> Self {
        Self {
            tag: addr.tag::<PageDataT>(),
            target: Some(target),
        }
    }

    pub fn tag_offset() -> usize {
        offset_of!(Self, tag)
    }

    pub fn target_offset() -> usize {
        offset_of!(Self, target)
    }

    pub fn size() -> usize {
        size_of::<Self>()
    }
}
