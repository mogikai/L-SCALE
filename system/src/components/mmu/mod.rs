pub mod tlb;
pub mod tlb_entry;

use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
    HVAddr,
};
use mogi_utilities::error::memory::MemoryError;
use std::ops::BitAnd;

use bitflags::bitflags;

// TODO: Extract memory error from utilities into system crate?

pub type MmuResult<T> = Result<T, MemoryError>;

// can this be simplified?
pub type PageFaultCallback = Box<dyn FnMut(EVAddr, MemoryAction) -> Option<(EVAddr, EPAddr, Permission)>>;

/// This trait represents a barebones abstraction of physical as well as virtual memory mapping
/// and read/write accessing. Through it, memory can be read using `EVAddr` as well as `EPAddr` and
/// page tables can be created and modified through `PageTableContextHandle` objects.
pub trait MMU {
    type MemoryContext;
    type PageData: PageMetaInfo;

    // A loaded context can be viewed as the emulated equivalent of a page table
    // currently loaded into cr3 (for x86). All virtual operations will take
    // effect on the currently loaded context

    /// Create a new context which refers to an initially blank page table.
    fn new_context(&self) -> Self::MemoryContext;

    /// Clone the currently active context
    fn clone_context(&self) -> Self::MemoryContext;

    /// Load the previously created `context` so that all future memory operations
    /// use the corresponding page table for address translation, mapping and permission checks.
    /// This needs to be used in conjunction with `self.flush_all()` to flush buffers
    /// when needed. Returns the old memory context.
    fn load_context(
        &mut self,
        context: &Self::MemoryContext,
    ) -> Self::MemoryContext;

    /// Flush the page table as well as the TLB region given by `addr` and `len`.
    fn flush_vaddr(
        &mut self,
        addr: EVAddr,
        len: usize,
    ) -> MmuResult<MemoryError>;

    /// Flush the whole page table as well as the TLB
    fn flush_all(&mut self) -> MmuResult<()>;

    // Point page table entry at `addr` to `paddr` with permissions `permissions`
    fn set_page_table_entry(
        &mut self,
        addr: EVAddr,
        paddr: EPAddr,
        permissions: Permission,
    ) -> MmuResult<()>;

    // Clear page table entry at `addr` so all future accesses will fail
    fn clear_page_table_entry(
        &mut self,
        addr: EVAddr,
    ) -> MmuResult<()>;

    /// Read bytes from the physical location specified by `addr` into `buffer`.
    fn read_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> MmuResult<()>;
    /// Write `data` to the physical location specified by `addr`.
    fn write_physical(
        &mut self,
        addr: EPAddr,
        data: &[u8],
    ) -> MmuResult<()>;
    // Fetch bytes for execution from the physical location specified by `addr` into `buffer`.
    fn fetch_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError>;

    /// Read bytes from the virtual location specified by `addr` into `buffer`.
    /// Will fail if no memory is mapped
    fn read(
        &mut self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> MmuResult<()>;
    /// Write `len` bytes from the virtual location specified by `addr`.
    /// Will fail if no memory is mapped or memory is readonly.
    fn write(
        &mut self,
        addr: EVAddr,
        data: &[u8],
    ) -> MmuResult<()>;
    // Fetch bytes for execution from the virtual location specified by `addr` into `buffer`.
    fn fetch(
        &mut self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError>;

    // TODO should physical memory allocation be part of the trait or be done reactively on e.g.
    // page fault by the MMU internally?
    /// Make `size` physical memory at address `addr` available to the emulator.
    fn alloc_physical(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> MmuResult<()>;

    /// Free `size` physical memory at address `addr` by cutting out.
    fn free_physical(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> MmuResult<()>;

    /// `hook` will be called whenever
    /// - an unmapped page is accessed
    /// - a readonly page is written to
    fn set_page_fault_callback(
        &mut self,
        callback: PageFaultCallback,
    ) -> MmuResult<()>;

    /// Resolve `addr` to the internal page address and put it into the appropriate tlb table.
    /// Also returns the resolved host address that points to the correct offset in the page
    /// to the just in time compiled code for direct access.
    unsafe fn populate_tlb<T>(
        &self,
        tlb: &mut T,
        addr: EVAddr,
        action: MemoryAction,
    ) -> Result<HVAddr<Self::PageData>, MemoryError>
    where
        T: TLB<PageData = Self::PageData>;
}

pub trait TLB {
    type PageData: PageMetaInfo;

    fn insert_read(
        &mut self,
        addr: EVAddr,
        page_addr: HVAddr<Self::PageData>,
    );

    fn insert_write(
        &mut self,
        addr: EVAddr,
        page_addr: HVAddr<Self::PageData>,
    );
}

pub trait PageMetaInfo {
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

bitflags! {
    #[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct Permission : u8 {
        const NONE = 0;
        const READ = 1;
        const WRITE = 2;
        const EXEC = 4;
        const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();
        const WRITE_EXEC = Self::WRITE.bits() | Self::EXEC.bits();
        const READ_EXEC = Self::READ.bits() | Self::EXEC.bits();
        const ALL = Self::READ.bits() | Self::WRITE.bits() | Self::EXEC.bits();
    }
}

bitflags! {
    #[allow(non_camel_case_types)]
    #[derive(Debug)]
    pub struct MREMAP_FLAGS: u64 {
        const MREMAP_MAYMOVE = 1;
        const MREMAP_FIXED = 2;
        const MREMAP_DONTUNMAP = 4;
    }
}

impl TryFrom<u64> for Permission {
    type Error = MemoryError;

    fn try_from(perm: u64) -> Result<Permission, MemoryError> {
        Permission::from_bits(u8::try_from(perm)?).ok_or(MemoryError::InvalidPermission)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
pub enum MemoryAction {
    READ,
    WRITE,
    FETCH,
    READ_UNMAPPED,
    WRITE_UNMAPPED,
    FETCH_UNMAPPED,
    WRITE_PROT,
    READ_PROT,
    FETCH_PROT,
    READ_AFTER,
}

impl MemoryAction {
    pub fn required_permissions(&self) -> Permission {
        match self {
            MemoryAction::READ => Permission::READ,
            MemoryAction::WRITE => Permission::WRITE,
            MemoryAction::FETCH => Permission::EXEC,
            _ => todo!(),
        }
    }
}
