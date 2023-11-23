use super::*;
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::memory::Memory;
use byteorder::ByteOrder;
use iced_x86::{
    Decoder,
    DecoderOptions,
    Formatter,
    Instruction,
    IntelFormatter,
};

pub(super) fn trace_hook<Endianness: ByteOrder + 'static>(
    uc: &mut (impl CPU + MMU),
    memory: &mut Memory<Endianness>,
    current_task: TaskId,
    address: u64,
    size: u32,
) {
    let accessor = memory.accessor(uc, current_task);
    let bytes: Vec<u8> = accessor
        .read_bytes(address.into(), size.try_into().unwrap())
        .unwrap();
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
        // let rdi: u64 = uc.reg_read(RegisterX86::RDI.into()).unwrap();
        // let rsp: u64 = uc.reg_read(RegisterX86::RSP.into()).unwrap();
        // let rax: u64 = uc.reg_read(RegisterX86::RAX.into()).unwrap();
        // let rbx: u64 = uc.reg_read(RegisterX86::RBX.into()).unwrap();
        // let rcx: u64 = uc.reg_read(RegisterX86::RCX.into()).unwrap();
        // let rdx: u64 = uc.reg_read(RegisterX86::RDX.into()).unwrap();
        // println!("RDI: {rdi:x}");
        // println!("RSP: {rsp:x}");
        // println!("RAX: {rax:x}");
        // println!("RBX: {rbx:x}");
        // println!("RCX: {rcx:x}");
        // println!("RDX: {rdx:x}");
    }
}
