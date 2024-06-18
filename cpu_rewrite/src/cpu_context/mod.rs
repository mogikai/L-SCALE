use mogi_mem::tlb::tlb::TLB;
use mogi_utilities::addresses::EVAddr;
use std::ptr::NonNull;

#[repr(C)]
pub struct CPUContext {
    registers: (),
    tlb: Option<NonNull<TLB>>,
}

impl CPUContext {
    pub fn new() -> Self {
        Self {
            registers: (),
            tlb: None,
        }
    }

    pub fn pc(&self) -> EVAddr {
        EVAddr::null()
    }
}
