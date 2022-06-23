use std::path::{Path, PathBuf};
use byteorder::LittleEndian;
use fuzzyemu::emulator::system::cpu::unicorn::UCWrapper;

use fuzzyemu::emulator::abstractions::emulator::Emulator;
use fuzzyemu::emulator::platform::linux::emulator::LinuxX86;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::memory::Memory;

fn main() {
    pretty_env_logger::init();
    let filesystem = Filesystem::new(Path::new("systems/ubuntu2004"));
    let memory = Memory::<LittleEndian>::new(0x4000000000000000, 0x2000000000000000);
    let cpu = UCWrapper::new(0);
    let mut emu = LinuxX86::new(filesystem, memory, cpu, PathBuf::from("/home/mark/main")).unwrap();
    emu.run().unwrap();
}