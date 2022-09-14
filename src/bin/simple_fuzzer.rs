use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use byteorder::LittleEndian;
use log::debug;

use fuzzyemu::emulator::abstractions::cpu::Hooks;
use fuzzyemu::emulator::abstractions::emulator::Emulator;
use fuzzyemu::emulator::platform::linux::emulator::LinuxX86;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernel;
use fuzzyemu::emulator::system::cpu::unicorn::UCWrapper;
use fuzzyemu::emulator::system::default_handles::ReadOnly;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::memory::Memory;

fn main() {
    pretty_env_logger::init();
    let mut blocks = HashMap::<(u64, u64, u64), u64>::new();
    let mut prev_block: u64 = 0;
    let mut prev_block2: u64 = 0;

    let filesystem = Rc::new(RefCell::new(Filesystem::from_disk(1_073_741_824, Path::new("systems/ubuntu2004")).unwrap()));
    let cpu = UCWrapper::new(0);
    let memory = Memory::<LittleEndian>::new(&mut *cpu.borrow_mut(), 0x4000000000000000, 10 * 1024 * 1024, 0x2000000000000000);
    let kernel = Rc::new(RefCell::new(LinuxKernel::new(memory, filesystem)));

    cpu.borrow_mut().add_block_hook(Box::new(|_, addr, _| {
        *blocks.entry((prev_block2, prev_block, addr)).or_insert(0) += 1;
        prev_block2 = prev_block;
        prev_block = addr;
        // TODO: Is this a performance bottleneck, do we have to use a simpler map?
    }));

    let mut data = vec![b'A'; 0x01];
    data.extend(vec![b'x', b'y', b'z', b'x', b'y', b'z', b'\n']);
    let demo_handle = Box::new(ReadOnly::from(data));
    kernel.borrow_mut().add_replacement_handle(0, demo_handle);
    let mut emu = LinuxX86::new(kernel, cpu, PathBuf::from("/home/mark/server_rel2")).unwrap();
    let result = emu.run();

    drop(emu);

    // debug!("Blocks: {:X?}", blocks);
    debug!("Unique 3-edges executed: {:?}", blocks.keys().clone().fold(0, |acc, _| acc + 1));
    debug!("Unique 2-edges executed: {:?}", blocks.keys().clone().map(|(_, b, c)| (*b, *c)).collect::<HashSet<(u64, u64)>>().iter().fold(0, |acc, _| acc + 1));
    debug!("Unique blocks executed: {:?}", blocks.keys().clone().map(|(_, _, c)| *c).collect::<HashSet<u64>>().iter().fold(0, |acc, _| acc + 1));
    debug!("Total 3-edges executed: {:?}", blocks.into_values().reduce(|acc, b| acc + b).unwrap());
    result.unwrap();
}