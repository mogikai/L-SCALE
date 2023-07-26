extern crate core;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use log::debug;

use fuzzyemu::emulator::abstractions::emulator::Emulator;
use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::hooks::Hooks;
use fuzzyemu::emulator::platform::linux::architecture::x86::X86;
use fuzzyemu::emulator::platform::linux::emulator::LinuxX86Scheduler;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernelTask;
use fuzzyemu::emulator::system::clock::Clock;
use fuzzyemu::emulator::system::fd_map::FileDescriptorMap;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::hardware::unicorn::UCWrapper;
use fuzzyemu::emulator::system::memory::Memory;
use fuzzyemu::emulator::system::System;
use fuzzyemu::LittleEndian;

fn main() -> Result<(), GenericError> {
    pretty_env_logger::init();
    let mut blocks = HashMap::<(u64, u64, u64), u64>::new();
    let mut prev_block: u64 = 0;
    let mut prev_block2: u64 = 0;
    let mut total_blocks: u64 = 0;

    let duration = {
        let cpu = UCWrapper::new();

        let filesystem = Filesystem::from_disk(1_073_741_824, Path::new("emulator/systems/ubuntu2304")).unwrap();
        let memory = Memory::new();
        let clock = Clock::new(Duration::from_nanos(1662647309301240322));
        let file_descriptor_map = FileDescriptorMap::new();
        let system = Rc::new(System::new(memory, filesystem, clock, file_descriptor_map));
        // let memory = Rc::new(RefCell::new(Memory::new(
        //     &mut *cpu.borrow_mut(),
        //     0x0000_7fff_ffff_ffff,
        //     10 * 1024 * 1024, // 10 MiB
        //     0x0000_5000_0000_0000,
        //     0x0000_3000_0000_0000,
        // )));

        // cpu.borrow_mut().add_block_hook(Box::new(|_, addr, _| {
        //     total_blocks += 1;
        //     *blocks.entry((prev_block2, prev_block, addr)).or_insert(0) += 1;
        //     prev_block2 = prev_block;
        //     prev_block = addr;
        //     // TODO: Is this a performance bottleneck, do we have to use a simpler map?
        // }));
        let vtlb_system = system.clone();
        cpu.borrow_mut()
            .add_vtlb_hook(Box::new(move |_uc, vaddr, _action| {
                vtlb_system.current_task.borrow().and_then(|task_id| {
                    vtlb_system
                        .memory
                        .borrow_mut()
                        .translate_vaddr(task_id, vaddr)
                        .ok()
                })
            }))
            .unwrap();
        // let code_system = system.clone();
        // cpu.borrow_mut()
        //     .add_code_hook(
        //         1.into(),
        //         0.into(),
        //         Box::new(move |uc, address, size| {
        //             let mut memory = code_system.memory.borrow_mut();
        //             _code_hook(uc, &mut memory, code_system.current_task.borrow().unwrap(), address, size)
        //         }),
        //     )
        //     .unwrap();

        // TODO: Only one task for now. Need to implement a scheduler that switches tasks.
        let initial_task = LinuxKernelTask::<LittleEndian, X86>::new(system.clone(), 1337, 1);

        // let mut data = vec![b'A'; 0x01];
        // data.extend(vec![b'x', b'y', b'z', b'x', b'y', b'z', b'\n']);
        // let demo_handle = Box::new(ReadOnly::from(data));
        // kernel.borrow_mut().add_replacement_handle(0, demo_handle);
        let args = [];
        let env = ["SHELL=/bin/sh", "LD_DEBUG=all", "LD_BIND_NOW=false"];
        // let eflags = cpu.borrow_mut().reg_read(RegisterX86::EFLAGS.into()).unwrap();
        // cpu.borrow_mut().reg_write(RegisterX86::EFLAGS.into(), eflags | (1 << 22)).unwrap();
        let mut scheduler = LinuxX86Scheduler::new(
            system.clone(),
            initial_task,
            cpu.clone(),
            PathBuf::from("/home/user/open62541_server_release"),
            &args,
            &env,
        )
        .unwrap();
        let start = Instant::now();
        let result = scheduler.run(None, None);
        debug!("Emulation returned {result:?}");
        start.elapsed()
    };

    debug!("BB/s: {:?}", total_blocks as f64 / duration.as_secs_f64());
    // debug!("Blocks: {:X?}", blocks);
    debug!("Unique 3-edges executed: {:?}", blocks.keys().fold(0, |acc, _| acc + 1));
    debug!(
        "Unique 2-edges executed: {:?}",
        blocks
            .keys()
            .map(|(_, b, c)| (*b, *c))
            .collect::<HashSet<(u64, u64)>>()
            .iter()
            .fold(0, |acc, _| acc + 1)
    );
    debug!(
        "Unique blocks executed: {:?}",
        blocks
            .keys()
            .map(|(_, _, c)| *c)
            .collect::<HashSet<u64>>()
            .iter()
            .fold(0, |acc, _| acc + 1)
    );
    debug!("Total 3-edges executed: {:?}", blocks.values().cloned().reduce(|acc, b| acc + b).unwrap());
    Ok(())
}

//
// fn _code_hook<'a, Endianness: ByteOrder>(
//     uc: &mut (impl CPU + MMU),
//     memory: &mut Memory<Endianness>,
//     current_task: TaskId,
//     address: u64,
//     size: u32,
// ) {
//     let accessor = memory.accessor(uc, current_task);
//     let bytes: Vec<u8> = accessor
//         .read_bytes(address.into(), size.try_into().unwrap())
//         .unwrap();
//     let mut decoder = Decoder::with_ip(64, &bytes, address, DecoderOptions::NONE);
//
//     // Formatters: Masm*, Nasm*, Gas* (AT&T) and Intel* (XED).
//     // For fastest code, see `SpecializedFormatter` which is ~3.3x faster. Use it if formatting
//     // speed is more important than being able to re-assemble formatted instructions.
//     let mut formatter = IntelFormatter::new();
//
//     // Change some options, there are many more
//     //	formatter.options_mut().set_digit_separator("`");
//     formatter.options_mut().set_first_operand_char_index(10);
//
//     // String implements FormatterOutput
//     let mut output = String::new();
//
//     // Initialize this outside the loop because decode_out() writes to every field
//     let mut instruction = Instruction::default();
//
//     // The decoder also implements Iterator/IntoIterator so you could use a for loop:
//     //	  for instruction in &mut decoder { /* ... */ }
//     // or collect():
//     //	  let instructions: Vec<_> = decoder.into_iter().collect();
//     // but can_decode()/decode_out() is a little faster:
//     while decoder.can_decode() {
//         // There's also a decode() method that returns an instruction but that also
//         // means it copies an instruction (40 bytes):
//         //	 instruction = decoder.decode();
//         decoder.decode_out(&mut instruction);
//
//         // Format the instruction ("disassemble" it)
//         output.clear();
//         formatter.format(&instruction, &mut output);
//
//         // Eg. "00007FFAC46ACDB2 488DAC2400FFFFFF	 lea	   rbp,[rsp-100h]"
//         print!("{:016X} ", instruction.ip());
//         let start_index = (instruction.ip() - address) as usize;
//         let instr_bytes = &bytes[start_index..start_index + instruction.len()];
//         for b in instr_bytes.iter() {
//             print!("{:02X}", b);
//         }
//         if instr_bytes.len() < 10 {
//             for _ in 0..10 - instr_bytes.len() {
//                 print!("  ");
//             }
//         }
//         println!(" {}", output);
//         // let rdi: u64 = uc.reg_read(RegisterX86::RDI.into()).unwrap();
//         // let rsp: u64 = uc.reg_read(RegisterX86::RSP.into()).unwrap();
//         // let rax: u64 = uc.reg_read(RegisterX86::RAX.into()).unwrap();
//         // let rbx: u64 = uc.reg_read(RegisterX86::RBX.into()).unwrap();
//         // let rcx: u64 = uc.reg_read(RegisterX86::RCX.into()).unwrap();
//         // let rdx: u64 = uc.reg_read(RegisterX86::RDX.into()).unwrap();
//         // println!("RDI: {rdi:x}");
//         // println!("RSP: {rsp:x}");
//         // println!("RAX: {rax:x}");
//         // println!("RBX: {rbx:x}");
//         // println!("RCX: {rcx:x}");
//         // println!("RDX: {rdx:x}");
//     }
// }
