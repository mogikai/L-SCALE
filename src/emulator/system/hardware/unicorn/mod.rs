use std::cell::RefCell;
use std::rc::Rc;

use generational_arena::Arena;
use unicorn_engine::unicorn_const::{Arch, Mode};
use unicorn_engine::{Context, Unicorn};

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::emulator::StopReason;
use crate::emulator::abstractions::mmu::MemoryAction;
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

pub mod cpu;
pub mod hooks;
pub mod mmu;

pub struct UCWrapper<'a> {
    uc: Option<Unicorn<'a, *mut UCWrapper<'a>>>,
    contexts: Arena<Context>,
    // TODO: Replace with signals / interrupts?
    stop_reason: Option<StopReason>,
    address_translate_callback:
        Option<Rc<RefCell<dyn FnMut(&mut Self, EVAddr, MemoryAction) -> Option<TLBEntry> + 'a>>>,
}

impl<'a> UCWrapper<'a> {
    pub fn new() -> Rc<RefCell<Self>> {
        let wrapper = Rc::new(RefCell::new(UCWrapper {
            uc: None,
            contexts: Arena::new(),
            stop_reason: None,
            address_translate_callback: None,
        }));
        wrapper.borrow_mut().uc = Some(Unicorn::new_with_data(Arch::X86, Mode::MODE_64, wrapper.as_ptr()).unwrap());
        // wrapper.borrow_mut().uc.as_mut().unwrap().add_code_hook(1, 0, _code_hook).unwrap();
        wrapper
    }

    fn uc_mut(&mut self) -> &mut Unicorn<'a, *mut UCWrapper<'a>> {
        self.uc.as_mut().unwrap()
    }

    fn uc(&self) -> &Unicorn<'a, *mut UCWrapper<'a>> {
        self.uc.as_ref().unwrap()
    }
}
