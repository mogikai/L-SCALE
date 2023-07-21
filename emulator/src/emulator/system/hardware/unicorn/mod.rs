use std::cell::RefCell;
use std::rc::Rc;

use generational_arena::Arena;
use iced_x86::Formatter;

use unicorn_engine::unicorn_const::{Arch, Mode};
use unicorn_engine::{Context, TlbType, Unicorn};

use crate::emulator::abstractions::emulator::StopReason;

pub mod cpu;
pub mod hooks;
pub mod mmu;

pub struct UCWrapper<'a> {
    uc: Option<Unicorn<'a, *mut UCWrapper<'a>>>,
    contexts: Arena<Context>,
    // TODO: Replace with signals / interrupts?
    stop_reason: Option<StopReason>,
}

impl<'a> UCWrapper<'a> {
    pub fn new() -> Rc<RefCell<Self>> {
        let wrapper = Rc::new(RefCell::new(UCWrapper {
            uc: None,
            contexts: Arena::new(),
            stop_reason: None,
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
