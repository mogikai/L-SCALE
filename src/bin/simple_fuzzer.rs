use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use byteorder::LittleEndian;
use log::trace;
use fuzzyemu::emulator::abstractions::cpu::{CPU, MMU, Hooks};

use fuzzyemu::emulator::abstractions::emulator::Emulator;
use fuzzyemu::emulator::platform::linux::emulator::LinuxX86;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernel;
use fuzzyemu::emulator::system::cpu::unicorn::UCWrapper;
use fuzzyemu::emulator::system::default_handles::ReadOnly;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::memory::Memory;

fn main() {
    pretty_env_logger::init();
    let filesystem = Filesystem::new(Path::new("systems/ubuntu2004"));
    let mut cpu = UCWrapper::new(0);
    let memory = Memory::<LittleEndian>::new(&mut *cpu.borrow_mut(), 0x4000000000000000, 10 * 1024 * 1024, 0x2000000000000000);
    let kernel = Rc::new(RefCell::new(LinuxKernel::new(memory, filesystem)));
    let mut data = vec![b'A'; 0x01];
    data.extend(vec![b'x', b'y', b'z', b'x', b'y', b'z', b'\n']);
    let demo_handle = Box::new(ReadOnly::from(data));
    kernel.borrow_mut().add_replacement_handle(0, demo_handle);
    let mut emu = LinuxX86::new(kernel, cpu, PathBuf::from("/home/mark/server_rel")).unwrap();
    emu.run().unwrap();
}