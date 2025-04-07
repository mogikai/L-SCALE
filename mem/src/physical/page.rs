use crate::PAGE_SIZE;
use mogi_system::components::mmu::{
    PageMetaInfo,
    Permission,
};
use mogi_utilities::addresses::HVAddr;
use std::fmt::{
    Debug,
    Formatter,
};
use std::rc::Rc;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Page {
    data: Rc<PageData>,

    /// Keeps track of whether this page has been written to.
    pub modified: bool,

    /// Keeps track of whether code within this page has been lifted.
    pub executed: bool,
    permission: Permission,
}

impl Page {
    pub fn new() -> Self {
        Self {
            data: Rc::new(PageData::zero()),
            modified: false,
            executed: false,
            permission: Permission::empty(),
        }
    }

    pub fn set_permission(
        &mut self,
        permission: Permission,
    ) {
        self.permission = permission;
    }

    pub fn tlb_entry_ptr(&self) -> HVAddr<PageData> {
        // We know this is safe, because self.data must point to something
        unsafe { HVAddr::new_unchecked(Rc::as_ptr(&self.data) as *mut PageData) }
    }

    pub fn data(&self) -> &[u8; PAGE_SIZE] {
        &self.data.data
    }

    pub fn data_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        &mut Rc::make_mut(&mut self.data).data
    }

    pub fn permissions(&self) -> Permission {
        self.permission
    }

    pub fn permissions_mut(&mut self) -> &mut Permission {
        &mut self.permission
    }
}

#[derive(Clone, Copy)]
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
impl Debug for PageData {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "PageData {{")?;
        for i in 0..self.data.len() {
            let str = format!("{:02X} ", self.data[i]);
            if i % 16 == 0 {
                write!(f, "\noffset: {:03X}     ", i)?;
            }
            write!(f, "{}", str)?;
        }
        write!(f, "\n}}")?;
        Ok(())
    }
}
