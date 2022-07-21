use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter};

use unicorn_engine::{InsnSysX86, RegisterX86, Unicorn};
use unicorn_engine::unicorn_const::{Arch, HookType, MemType, Mode, Permission, uc_error};

use crate::emulator::abstractions::cpu::{CPU, CPUError, Hooks, MemoryAction, MMU, Register, Vaddr};
use crate::emulator::abstractions::cpu;

impl From<uc_error> for CPUError {
    fn from(e: uc_error) -> Self {
        trace!("UC ERROR: {:?}", e);
        CPUError::Unknown
    }
}

pub struct UCWrapper<'a> {
    uc: Option<Unicorn<'a, *mut UCWrapper<'a>>>,
}

fn _code_hook<'a>(uc: &mut Unicorn<'a, *mut UCWrapper<'_>>, address: u64, size: u32)
{
    let bytes: Vec<u8> = uc.mem_read_as_vec(address, size.try_into().unwrap()).unwrap();
    let mut decoder =
        Decoder::with_ip(64, &bytes, address, DecoderOptions::NONE);

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
    }
}

impl<'a> UCWrapper<'a> {
    pub fn new(_: u64) -> Rc<RefCell<Self>> {
        let wrapper = Rc::new(RefCell::new(
            UCWrapper {
                uc: None,
            }
        ));
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

impl CPU for UCWrapper<'_> {
    fn emulate(&mut self, stop: Option<Vaddr>, timeout: Option<Duration>, instruction_count: Option<usize>) -> Result<(), CPUError> {
        let rip = self.uc().reg_read(RegisterX86::RIP)?;
        let timeout: u64 = if let Some(timeout) = timeout { timeout.as_micros().try_into()? } else { 0u64 };
        Ok(self.uc_mut().emu_start(rip, stop.unwrap_or(0), timeout, instruction_count.unwrap_or(0))?)
    }

    fn reg_write(&mut self, register: Register, value: u64) -> Result<(), CPUError> {
        let register = RegisterX86::try_from(register)?;
        Ok(self.uc_mut().reg_write(register, value)?)
    }

    fn reg_read(&self, register: Register) -> Result<u64, CPUError> {
        let register = RegisterX86::try_from(register)?;
        Ok(self.uc().reg_read(register)?)
    }
}

impl MMU for UCWrapper<'_> {
    unsafe fn mem_map_ptr(&mut self, address: Vaddr, size: usize, perms: cpu::Permission, ptr: u64) -> Result<(), CPUError> {
        Ok(self.uc_mut().mem_map_ptr(address, size, perms.try_into()?, ptr as _)?)
    }

    fn mem_unmap(&mut self, address: Vaddr, size: usize) -> Result<(), CPUError> {
        Ok(self.uc_mut().mem_unmap(address, size)?)
    }
}

impl<'a> Hooks<'a> for UCWrapper<'a>{
    fn add_syscall_hook(&mut self, start: Vaddr, end: Vaddr, mut hook: Box<dyn FnMut(&mut Self) + 'a>) {
        // The trampoline takes ownership of the hook. The trampoline is then owned by the unicorn instance for as long
        // as the UCWrapper instance is alive.
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>| {
            // This is safe because the lifetime of unicorn and UCWrapper have a  1:1 relationship
            // Mutable access is also fine, since the unicorn callback is expected
            // to mutate stuff in the unicorn instance during callbacks.
            // The only other mutable access can come from UCWrapper itself
            let data = unsafe { &mut **uc.get_data_mut() };
            hook(data);
        };
        let _hook = self.uc_mut().add_insn_sys_hook(InsnSysX86::SYSCALL, start, end, trampoline).expect("failed to add syscall hook");
    }

    fn add_unmapped_mem_hook(&mut self, start: Vaddr, end: Vaddr, mut hook: Box<dyn FnMut(&mut Self, MemoryAction, u64, usize, i64) -> bool + 'a>) {
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>, mem_type, address, size, value| {
            let data = unsafe { &mut **uc.get_data_mut() };
            hook(data, MemoryAction::from(mem_type), address, size, value)
        };
        let _hook = self.uc_mut().add_mem_hook(HookType::MEM_UNMAPPED, start, end, trampoline).unwrap();
    }
}

impl From<MemType> for MemoryAction {
    fn from(memtype: MemType) -> Self {
        match memtype {
            MemType::READ => MemoryAction::READ,
            MemType::WRITE => MemoryAction::WRITE,
            MemType::FETCH => MemoryAction::FETCH,
            MemType::READ_UNMAPPED => MemoryAction::READ_UNMAPPED,
            MemType::WRITE_UNMAPPED => MemoryAction::WRITE_UNMAPPED,
            MemType::FETCH_UNMAPPED => MemoryAction::FETCH_UNMAPPED,
            MemType::WRITE_PROT => MemoryAction::WRITE_PROT,
            MemType::READ_PROT => MemoryAction::READ_PROT,
            MemType::FETCH_PROT => MemoryAction::FETCH_PROT,
            MemType::READ_AFTER => MemoryAction::READ_AFTER,
        }
    }
}

impl TryFrom<cpu::Permission> for Permission {
    type Error = CPUError;

    fn try_from(perm: cpu::Permission) -> Result<Permission, CPUError> {
        Permission::from_bits(perm.bits()).ok_or(CPUError::Unknown)
    }
}

impl TryFrom<Register> for RegisterX86 {
    type Error = CPUError;

    fn try_from(reg: Register) -> Result<Self, Self::Error> {
        if let Register::X86(reg) = reg {
            Ok(
                match reg {
                    cpu::RegisterX86::INVALID => RegisterX86::INVALID,
                    cpu::RegisterX86::AH => RegisterX86::AH,
                    cpu::RegisterX86::AL => RegisterX86::AL,
                    cpu::RegisterX86::AX => RegisterX86::AX,
                    cpu::RegisterX86::BH => RegisterX86::BH,
                    cpu::RegisterX86::BL => RegisterX86::BL,
                    cpu::RegisterX86::BP => RegisterX86::BP,
                    cpu::RegisterX86::BPL => RegisterX86::BPL,
                    cpu::RegisterX86::BX => RegisterX86::BX,
                    cpu::RegisterX86::CH => RegisterX86::CH,
                    cpu::RegisterX86::CL => RegisterX86::CL,
                    cpu::RegisterX86::CS => RegisterX86::CS,
                    cpu::RegisterX86::CX => RegisterX86::CX,
                    cpu::RegisterX86::DH => RegisterX86::DH,
                    cpu::RegisterX86::DI => RegisterX86::DI,
                    cpu::RegisterX86::DIL => RegisterX86::DIL,
                    cpu::RegisterX86::DL => RegisterX86::DL,
                    cpu::RegisterX86::DS => RegisterX86::DS,
                    cpu::RegisterX86::DX => RegisterX86::DX,
                    cpu::RegisterX86::EAX => RegisterX86::EAX,
                    cpu::RegisterX86::EBP => RegisterX86::EBP,
                    cpu::RegisterX86::EBX => RegisterX86::EBX,
                    cpu::RegisterX86::ECX => RegisterX86::ECX,
                    cpu::RegisterX86::EDI => RegisterX86::EDI,
                    cpu::RegisterX86::EDX => RegisterX86::EDX,
                    cpu::RegisterX86::EFLAGS => RegisterX86::EFLAGS,
                    cpu::RegisterX86::EIP => RegisterX86::EIP,
                    cpu::RegisterX86::ES => RegisterX86::ES,
                    cpu::RegisterX86::ESI => RegisterX86::ESI,
                    cpu::RegisterX86::ESP => RegisterX86::ESP,
                    cpu::RegisterX86::FPSW => RegisterX86::FPSW,
                    cpu::RegisterX86::FS => RegisterX86::FS,
                    cpu::RegisterX86::GS => RegisterX86::GS,
                    cpu::RegisterX86::IP => RegisterX86::IP,
                    cpu::RegisterX86::RAX => RegisterX86::RAX,
                    cpu::RegisterX86::RBP => RegisterX86::RBP,
                    cpu::RegisterX86::RBX => RegisterX86::RBX,
                    cpu::RegisterX86::RCX => RegisterX86::RCX,
                    cpu::RegisterX86::RDI => RegisterX86::RDI,
                    cpu::RegisterX86::RDX => RegisterX86::RDX,
                    cpu::RegisterX86::RIP => RegisterX86::RIP,
                    cpu::RegisterX86::RSI => RegisterX86::RSI,
                    cpu::RegisterX86::RSP => RegisterX86::RSP,
                    cpu::RegisterX86::SI => RegisterX86::SI,
                    cpu::RegisterX86::SIL => RegisterX86::SIL,
                    cpu::RegisterX86::SP => RegisterX86::SP,
                    cpu::RegisterX86::SPL => RegisterX86::SPL,
                    cpu::RegisterX86::SS => RegisterX86::SS,
                    cpu::RegisterX86::CR0 => RegisterX86::CR0,
                    cpu::RegisterX86::CR1 => RegisterX86::CR1,
                    cpu::RegisterX86::CR2 => RegisterX86::CR2,
                    cpu::RegisterX86::CR3 => RegisterX86::CR3,
                    cpu::RegisterX86::CR4 => RegisterX86::CR4,
                    cpu::RegisterX86::CR8 => RegisterX86::CR8,
                    cpu::RegisterX86::DR0 => RegisterX86::DR0,
                    cpu::RegisterX86::DR1 => RegisterX86::DR1,
                    cpu::RegisterX86::DR2 => RegisterX86::DR2,
                    cpu::RegisterX86::DR3 => RegisterX86::DR3,
                    cpu::RegisterX86::DR4 => RegisterX86::DR4,
                    cpu::RegisterX86::DR5 => RegisterX86::DR5,
                    cpu::RegisterX86::DR6 => RegisterX86::DR6,
                    cpu::RegisterX86::DR7 => RegisterX86::DR7,
                    cpu::RegisterX86::FP0 => RegisterX86::FP0,
                    cpu::RegisterX86::FP1 => RegisterX86::FP1,
                    cpu::RegisterX86::FP2 => RegisterX86::FP2,
                    cpu::RegisterX86::FP3 => RegisterX86::FP3,
                    cpu::RegisterX86::FP4 => RegisterX86::FP4,
                    cpu::RegisterX86::FP5 => RegisterX86::FP5,
                    cpu::RegisterX86::FP6 => RegisterX86::FP6,
                    cpu::RegisterX86::FP7 => RegisterX86::FP7,
                    cpu::RegisterX86::K0 => RegisterX86::K0,
                    cpu::RegisterX86::K1 => RegisterX86::K1,
                    cpu::RegisterX86::K2 => RegisterX86::K2,
                    cpu::RegisterX86::K3 => RegisterX86::K3,
                    cpu::RegisterX86::K4 => RegisterX86::K4,
                    cpu::RegisterX86::K5 => RegisterX86::K5,
                    cpu::RegisterX86::K6 => RegisterX86::K6,
                    cpu::RegisterX86::K7 => RegisterX86::K7,
                    cpu::RegisterX86::MM0 => RegisterX86::MM0,
                    cpu::RegisterX86::MM1 => RegisterX86::MM1,
                    cpu::RegisterX86::MM2 => RegisterX86::MM2,
                    cpu::RegisterX86::MM3 => RegisterX86::MM3,
                    cpu::RegisterX86::MM4 => RegisterX86::MM4,
                    cpu::RegisterX86::MM5 => RegisterX86::MM5,
                    cpu::RegisterX86::MM6 => RegisterX86::MM6,
                    cpu::RegisterX86::MM7 => RegisterX86::MM7,
                    cpu::RegisterX86::R8 => RegisterX86::R8,
                    cpu::RegisterX86::R9 => RegisterX86::R9,
                    cpu::RegisterX86::R10 => RegisterX86::R10,
                    cpu::RegisterX86::R11 => RegisterX86::R11,
                    cpu::RegisterX86::R12 => RegisterX86::R12,
                    cpu::RegisterX86::R13 => RegisterX86::R13,
                    cpu::RegisterX86::R14 => RegisterX86::R14,
                    cpu::RegisterX86::R15 => RegisterX86::R15,
                    cpu::RegisterX86::ST0 => RegisterX86::ST0,
                    cpu::RegisterX86::ST1 => RegisterX86::ST1,
                    cpu::RegisterX86::ST2 => RegisterX86::ST2,
                    cpu::RegisterX86::ST3 => RegisterX86::ST3,
                    cpu::RegisterX86::ST4 => RegisterX86::ST4,
                    cpu::RegisterX86::ST5 => RegisterX86::ST5,
                    cpu::RegisterX86::ST6 => RegisterX86::ST6,
                    cpu::RegisterX86::ST7 => RegisterX86::ST7,
                    cpu::RegisterX86::XMM0 => RegisterX86::XMM0,
                    cpu::RegisterX86::XMM1 => RegisterX86::XMM1,
                    cpu::RegisterX86::XMM2 => RegisterX86::XMM2,
                    cpu::RegisterX86::XMM3 => RegisterX86::XMM3,
                    cpu::RegisterX86::XMM4 => RegisterX86::XMM4,
                    cpu::RegisterX86::XMM5 => RegisterX86::XMM5,
                    cpu::RegisterX86::XMM6 => RegisterX86::XMM6,
                    cpu::RegisterX86::XMM7 => RegisterX86::XMM7,
                    cpu::RegisterX86::XMM8 => RegisterX86::XMM8,
                    cpu::RegisterX86::XMM9 => RegisterX86::XMM9,
                    cpu::RegisterX86::XMM10 => RegisterX86::XMM10,
                    cpu::RegisterX86::XMM11 => RegisterX86::XMM11,
                    cpu::RegisterX86::XMM12 => RegisterX86::XMM12,
                    cpu::RegisterX86::XMM13 => RegisterX86::XMM13,
                    cpu::RegisterX86::XMM14 => RegisterX86::XMM14,
                    cpu::RegisterX86::XMM15 => RegisterX86::XMM15,
                    cpu::RegisterX86::XMM16 => RegisterX86::XMM16,
                    cpu::RegisterX86::XMM17 => RegisterX86::XMM17,
                    cpu::RegisterX86::XMM18 => RegisterX86::XMM18,
                    cpu::RegisterX86::XMM19 => RegisterX86::XMM19,
                    cpu::RegisterX86::XMM20 => RegisterX86::XMM20,
                    cpu::RegisterX86::XMM21 => RegisterX86::XMM21,
                    cpu::RegisterX86::XMM22 => RegisterX86::XMM22,
                    cpu::RegisterX86::XMM23 => RegisterX86::XMM23,
                    cpu::RegisterX86::XMM24 => RegisterX86::XMM24,
                    cpu::RegisterX86::XMM25 => RegisterX86::XMM25,
                    cpu::RegisterX86::XMM26 => RegisterX86::XMM26,
                    cpu::RegisterX86::XMM27 => RegisterX86::XMM27,
                    cpu::RegisterX86::XMM28 => RegisterX86::XMM28,
                    cpu::RegisterX86::XMM29 => RegisterX86::XMM29,
                    cpu::RegisterX86::XMM30 => RegisterX86::XMM30,
                    cpu::RegisterX86::XMM31 => RegisterX86::XMM31,
                    cpu::RegisterX86::YMM0 => RegisterX86::YMM0,
                    cpu::RegisterX86::YMM1 => RegisterX86::YMM1,
                    cpu::RegisterX86::YMM2 => RegisterX86::YMM2,
                    cpu::RegisterX86::YMM3 => RegisterX86::YMM3,
                    cpu::RegisterX86::YMM4 => RegisterX86::YMM4,
                    cpu::RegisterX86::YMM5 => RegisterX86::YMM5,
                    cpu::RegisterX86::YMM6 => RegisterX86::YMM6,
                    cpu::RegisterX86::YMM7 => RegisterX86::YMM7,
                    cpu::RegisterX86::YMM8 => RegisterX86::YMM8,
                    cpu::RegisterX86::YMM9 => RegisterX86::YMM9,
                    cpu::RegisterX86::YMM10 => RegisterX86::YMM10,
                    cpu::RegisterX86::YMM11 => RegisterX86::YMM11,
                    cpu::RegisterX86::YMM12 => RegisterX86::YMM12,
                    cpu::RegisterX86::YMM13 => RegisterX86::YMM13,
                    cpu::RegisterX86::YMM14 => RegisterX86::YMM14,
                    cpu::RegisterX86::YMM15 => RegisterX86::YMM15,
                    cpu::RegisterX86::YMM16 => RegisterX86::YMM16,
                    cpu::RegisterX86::YMM17 => RegisterX86::YMM17,
                    cpu::RegisterX86::YMM18 => RegisterX86::YMM18,
                    cpu::RegisterX86::YMM19 => RegisterX86::YMM19,
                    cpu::RegisterX86::YMM20 => RegisterX86::YMM20,
                    cpu::RegisterX86::YMM21 => RegisterX86::YMM21,
                    cpu::RegisterX86::YMM22 => RegisterX86::YMM22,
                    cpu::RegisterX86::YMM23 => RegisterX86::YMM23,
                    cpu::RegisterX86::YMM24 => RegisterX86::YMM24,
                    cpu::RegisterX86::YMM25 => RegisterX86::YMM25,
                    cpu::RegisterX86::YMM26 => RegisterX86::YMM26,
                    cpu::RegisterX86::YMM27 => RegisterX86::YMM27,
                    cpu::RegisterX86::YMM28 => RegisterX86::YMM28,
                    cpu::RegisterX86::YMM29 => RegisterX86::YMM29,
                    cpu::RegisterX86::YMM30 => RegisterX86::YMM30,
                    cpu::RegisterX86::YMM31 => RegisterX86::YMM31,
                    cpu::RegisterX86::ZMM0 => RegisterX86::ZMM0,
                    cpu::RegisterX86::ZMM1 => RegisterX86::ZMM1,
                    cpu::RegisterX86::ZMM2 => RegisterX86::ZMM2,
                    cpu::RegisterX86::ZMM3 => RegisterX86::ZMM3,
                    cpu::RegisterX86::ZMM4 => RegisterX86::ZMM4,
                    cpu::RegisterX86::ZMM5 => RegisterX86::ZMM5,
                    cpu::RegisterX86::ZMM6 => RegisterX86::ZMM6,
                    cpu::RegisterX86::ZMM7 => RegisterX86::ZMM7,
                    cpu::RegisterX86::ZMM8 => RegisterX86::ZMM8,
                    cpu::RegisterX86::ZMM9 => RegisterX86::ZMM9,
                    cpu::RegisterX86::ZMM10 => RegisterX86::ZMM10,
                    cpu::RegisterX86::ZMM11 => RegisterX86::ZMM11,
                    cpu::RegisterX86::ZMM12 => RegisterX86::ZMM12,
                    cpu::RegisterX86::ZMM13 => RegisterX86::ZMM13,
                    cpu::RegisterX86::ZMM14 => RegisterX86::ZMM14,
                    cpu::RegisterX86::ZMM15 => RegisterX86::ZMM15,
                    cpu::RegisterX86::ZMM16 => RegisterX86::ZMM16,
                    cpu::RegisterX86::ZMM17 => RegisterX86::ZMM17,
                    cpu::RegisterX86::ZMM18 => RegisterX86::ZMM18,
                    cpu::RegisterX86::ZMM19 => RegisterX86::ZMM19,
                    cpu::RegisterX86::ZMM20 => RegisterX86::ZMM20,
                    cpu::RegisterX86::ZMM21 => RegisterX86::ZMM21,
                    cpu::RegisterX86::ZMM22 => RegisterX86::ZMM22,
                    cpu::RegisterX86::ZMM23 => RegisterX86::ZMM23,
                    cpu::RegisterX86::ZMM24 => RegisterX86::ZMM24,
                    cpu::RegisterX86::ZMM25 => RegisterX86::ZMM25,
                    cpu::RegisterX86::ZMM26 => RegisterX86::ZMM26,
                    cpu::RegisterX86::ZMM27 => RegisterX86::ZMM27,
                    cpu::RegisterX86::ZMM28 => RegisterX86::ZMM28,
                    cpu::RegisterX86::ZMM29 => RegisterX86::ZMM29,
                    cpu::RegisterX86::ZMM30 => RegisterX86::ZMM30,
                    cpu::RegisterX86::ZMM31 => RegisterX86::ZMM31,
                    cpu::RegisterX86::R8B => RegisterX86::R8B,
                    cpu::RegisterX86::R9B => RegisterX86::R9B,
                    cpu::RegisterX86::R10B => RegisterX86::R10B,
                    cpu::RegisterX86::R11B => RegisterX86::R11B,
                    cpu::RegisterX86::R12B => RegisterX86::R12B,
                    cpu::RegisterX86::R13B => RegisterX86::R13B,
                    cpu::RegisterX86::R14B => RegisterX86::R14B,
                    cpu::RegisterX86::R15B => RegisterX86::R15B,
                    cpu::RegisterX86::R8D => RegisterX86::R8D,
                    cpu::RegisterX86::R9D => RegisterX86::R9D,
                    cpu::RegisterX86::R10D => RegisterX86::R10D,
                    cpu::RegisterX86::R11D => RegisterX86::R11D,
                    cpu::RegisterX86::R12D => RegisterX86::R12D,
                    cpu::RegisterX86::R13D => RegisterX86::R13D,
                    cpu::RegisterX86::R14D => RegisterX86::R14D,
                    cpu::RegisterX86::R15D => RegisterX86::R15D,
                    cpu::RegisterX86::R8W => RegisterX86::R8W,
                    cpu::RegisterX86::R9W => RegisterX86::R9W,
                    cpu::RegisterX86::R10W => RegisterX86::R10W,
                    cpu::RegisterX86::R11W => RegisterX86::R11W,
                    cpu::RegisterX86::R12W => RegisterX86::R12W,
                    cpu::RegisterX86::R13W => RegisterX86::R13W,
                    cpu::RegisterX86::R14W => RegisterX86::R14W,
                    cpu::RegisterX86::R15W => RegisterX86::R15W,
                    cpu::RegisterX86::IDTR => RegisterX86::IDTR,
                    cpu::RegisterX86::GDTR => RegisterX86::GDTR,
                    cpu::RegisterX86::LDTR => RegisterX86::LDTR,
                    cpu::RegisterX86::TR => RegisterX86::TR,
                    cpu::RegisterX86::FPCW => RegisterX86::FPCW,
                    cpu::RegisterX86::FPTAG => RegisterX86::FPTAG,
                    cpu::RegisterX86::MSR => RegisterX86::MSR,
                    cpu::RegisterX86::MXCSR => RegisterX86::MXCSR,
                    cpu::RegisterX86::FS_BASE => RegisterX86::FS_BASE,
                    cpu::RegisterX86::GS_BASE => RegisterX86::GS_BASE,
                    cpu::RegisterX86::FLAGS => RegisterX86::FLAGS,
                    cpu::RegisterX86::RFLAGS => RegisterX86::RFLAGS,
                    cpu::RegisterX86::FIP => RegisterX86::FIP,
                    cpu::RegisterX86::FCS => RegisterX86::FCS,
                    cpu::RegisterX86::FDP => RegisterX86::FDP,
                    cpu::RegisterX86::FDS => RegisterX86::FDS,
                    cpu::RegisterX86::FOP => RegisterX86::FOP,
                    cpu::RegisterX86::ENDING => RegisterX86::ENDING,
                }
            )
        } else {
            Err(CPUError::InvalidArch)
        }
    }
}