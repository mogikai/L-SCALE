use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use iced_x86::Formatter;
use unicorn_engine::unicorn_const::{
    Arch,
    Mode,
};
use unicorn_engine::{
    Context,
    TlbType,
    UcHookId,
    Unicorn,
};

use mogi_system::errors::cpu::StopReason;
use mogi_utilities::addresses::EVAddr;

pub mod cpu;
pub mod hooks;
pub mod mmu;
mod restorable;
mod tests;

#[derive(Default, Debug)]
pub struct UCWrapper<'a> {
    uc: Option<Unicorn<'a, *mut UCWrapper<'a>>>,
    breakpoints: BTreeMap<EVAddr, UcHookId>,
    contexts: BTreeMap<u64, Context>,
    next_key: u64,
    // TODO: Replace with signals / interrupts?
    stop_reason: Option<StopReason>,
    snapshots: Vec<Snapshot>,
}

#[derive(Debug)]
struct Snapshot {
    uc_context: Context,
    contexts: BTreeMap<u64, Context>,
    next_key: u64,
}

impl<'a> UCWrapper<'a> {
    pub fn new() -> Rc<RefCell<Self>> {
        let wrapper = Rc::new(RefCell::new(UCWrapper { ..Default::default() }));
        wrapper.borrow_mut().uc = Some(Unicorn::new_with_data(Arch::X86, Mode::MODE_64, wrapper.as_ptr()).unwrap());
        if let Some(uc) = wrapper.borrow_mut().uc.as_ref() {
            uc.ctl_tlb_type(TlbType::VIRTUAL).unwrap()
        }
        wrapper
    }

    fn uc_mut(&mut self) -> &mut Unicorn<'a, *mut UCWrapper<'a>> {
        self.uc.as_mut().unwrap()
    }

    fn uc(&self) -> &Unicorn<'a, *mut UCWrapper<'a>> {
        self.uc.as_ref().unwrap()
    }
}
