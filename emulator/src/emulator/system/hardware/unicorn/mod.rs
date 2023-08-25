use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use generational_arena::Arena;
use iced_x86::Formatter;

use crate::emulator::abstractions::cpu::address::EVAddr;
use unicorn_engine::unicorn_const::{Arch, Mode};
use unicorn_engine::{Context, TlbType, UcHookId, Unicorn};

use crate::emulator::abstractions::emulator::StopReason;

pub mod cpu;
pub mod hooks;
pub mod mmu;
mod restorable;
mod tests;

pub struct UCWrapper<'a> {
    uc: Option<Unicorn<'a, *mut UCWrapper<'a>>>,
    breakpoints: BTreeMap<EVAddr, UcHookId>,
    /// The Context is wrapped in an Rc because this makes saving a copy of the contexts arena easy.
    contexts: Arena<Context>,
    // TODO: Replace with signals / interrupts?
    stop_reason: Option<StopReason>,
    snapshots: Vec<Snapshot>,
}

struct Snapshot {
    uc_context: Context,
}

impl<'a> UCWrapper<'a> {
    pub fn new() -> Rc<RefCell<Self>> {
        let wrapper = Rc::new(RefCell::new(UCWrapper {
            uc: None,
            breakpoints: Default::default(),
            contexts: Arena::new(),
            stop_reason: None,
            snapshots: vec![],
        }));
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
