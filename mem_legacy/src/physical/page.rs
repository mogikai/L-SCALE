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
}
/*

0x100 -> p0x8000 -> PageData(R)

0x500 -> p0x8000 |

page_faulthook {
    let paddr = get_free_physical(PAGE_SIZE);
    mapping.update(addr, paddr);
    return paddr;
}

 */
impl Page {
    pub fn new() -> Self {
        Self {
            data: Rc::new(PageData::zero()),
            modified: false,
            executed: false,
        }
    }

    pub fn set_permissions(
        &mut self,
        permission: Permission,
    ) {
        Rc::make_mut(&mut self.data)
            .permissions
            .iter_mut()
            .for_each(|perm| {
                *perm = permission;
            })
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

    pub fn permissions(&self) -> &[Permission; PAGE_SIZE] {
        &self.data.permissions
    }

    pub fn permissions_mut(&mut self) -> &mut [Permission; PAGE_SIZE] {
        &mut Rc::make_mut(&mut self.data).permissions
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
#[repr(align(4096))]
pub struct PageData {
    // TODO: Remove pub
    pub data: [u8; PAGE_SIZE],
    // TODO: Move permissions to page table
    pub permissions: [Permission; PAGE_SIZE],
}

impl PageMetaInfo for PageData {
    fn page_size() -> usize {
        todo!()
    }

    fn page_offset_bits() -> usize {
        todo!()
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
            let str = match self.permissions[i] {
                Permission::READ => ansi_term::Colour::Green.paint(str),
                Permission::WRITE => ansi_term::Colour::Blue.paint(str),
                Permission::EXEC => ansi_term::Colour::Red.paint(str),
                Permission::READ_WRITE => ansi_term::Colour::Cyan.paint(str),
                Permission::READ_EXEC => ansi_term::Colour::Yellow.paint(str),
                Permission::WRITE_EXEC => ansi_term::Colour::Purple.paint(str),
                _ => ansi_term::Colour::Black.paint(str),
            };
            if i % 16 == 0 {
                write!(f, "\noffset: {:03X}     ", i)?;
            }
            write!(f, "{}", str)?;
        }
        write!(f, "\n}}")?;
        Ok(())
    }
}

impl PageData {
    pub fn zero() -> Self {
        Self {
            data: [0u8; PAGE_SIZE],
            permissions: [Default::default(); PAGE_SIZE],
        }
    }
}
