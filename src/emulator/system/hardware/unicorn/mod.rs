use std::cell::RefCell;
use std::rc::Rc;

use generational_arena::Arena;
use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter};
use unicorn_engine::unicorn_const::{Arch, Mode};
use unicorn_engine::{Context, TlbType, Unicorn};

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

fn _code_hook<'a>(
    uc: &mut Unicorn<'a, *mut UCWrapper<'_>>,
    address: u64,
    size: u32,
) {
    let bytes: Vec<u8> = uc.mem_read_as_vec(address, size.try_into().unwrap()).unwrap();
    let mut decoder = Decoder::with_ip(64, &bytes, address, DecoderOptions::NONE);

    // Formatters: Masm*, Nasm*, Gas* (AT&T) and Intel* (XED).
    // For fastest code, see `SpecializedFormatter` which is ~3.3x faster. Use it if formatting
    // speed is more important than being able to re-assemble formatted instructions.
    let mut formatter = IntelFormatter::new();

    // Change some options, there are many more
    //	formatter.options_mut().set_digit_separator("`");
    formatter.options_mut().set_first_operand_char_index(10);

    // String implements FormatterOutput
    let mut output = String::new();

    // Initialize this outside the loop because decode_out() writes to every field
    let mut instruction = Instruction::default();

    // The decoder also implements Iterator/IntoIterator so you could use a for loop:
    //	  for instruction in &mut decoder { /* ... */ }
    // or collect():
    //	  let instructions: Vec<_> = decoder.into_iter().collect();
    // but can_decode()/decode_out() is a little faster:
    while decoder.can_decode() {
        // There's also a decode() method that returns an instruction but that also
        // means it copies an instruction (40 bytes):
        //	 instruction = decoder.decode();
        decoder.decode_out(&mut instruction);

        // Format the instruction ("disassemble" it)
        output.clear();
        formatter.format(&instruction, &mut output);

        // Eg. "00007FFAC46ACDB2 488DAC2400FFFFFF	 lea	   rbp,[rsp-100h]"
        print!("{:016X} ", instruction.ip());
        let start_index = (instruction.ip() - address) as usize;
        let instr_bytes = &bytes[start_index..start_index + instruction.len()];
        for b in instr_bytes.iter() {
            print!("{:02X}", b);
        }
        if instr_bytes.len() < 10 {
            for _ in 0..10 - instr_bytes.len() {
                print!("  ");
            }
        }
        println!(" {}", output);
        // let rsp: u64 = uc.reg_read(unicorn_engine::RegisterX86::RSP).unwrap();
        // let rax: u64 = uc.reg_read(unicorn_engine::RegisterX86::RAX).unwrap();
        // let rbx: u64 = uc.reg_read(unicorn_engine::RegisterX86::RBX).unwrap();
        // let rcx: u64 = uc.reg_read(unicorn_engine::RegisterX86::RCX).unwrap();
        // let rdx: u64 = uc.reg_read(unicorn_engine::RegisterX86::RDX).unwrap();
        // println!("RSP: {rsp:x}");
        // println!("RAX: {rax:x}");
        // println!("RBX: {rbx:x}");
        // println!("RCX: {rcx:x}");
        // println!("RDX: {rdx:x}");
    }
}
