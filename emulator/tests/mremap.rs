use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use byteorder::LittleEndian;

use fuzzyemu::emulator::abstractions::emulator::{Scheduler, SchedulerError, StopReason};
use fuzzyemu::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use fuzzyemu::emulator::abstractions::hooks::Hooks;
use fuzzyemu::emulator::platform::linux::architecture::x86::X86;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernelTask;
use fuzzyemu::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use fuzzyemu::emulator::system::clock::Clock;
use fuzzyemu::emulator::system::fd_map::FileDescriptorMap;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::hardware::unicorn::UCWrapper;
use fuzzyemu::emulator::system::memory::Memory;
use fuzzyemu::emulator::system::System;

#[test]
fn remap_private_anon() -> Result<(), GenericError> {
    pretty_env_logger::init();

    let cpu = UCWrapper::new();

    let filesystem = Filesystem::from_disk(1_073_741_824, Path::new("systems/ubuntu2204")).unwrap();
    let memory = Memory::new();
    let clock = Clock::new(Duration::from_nanos(1662647309301240322));
    let fd_map = FileDescriptorMap::new();
    let system = System::new(memory, filesystem, clock, fd_map);

    // TODO: Only one task for now. Need to implement a scheduler that switches tasks.
    let initial_task = LinuxKernelTask::<LittleEndian, X86>::new(system.clone(), 1337, 1);

    let args = [];
    let env = ["SHELL=/bin/sh", "LD_BIND_NOW=true"];
    let mut scheduler = LinuxX86Scheduler::new(
        system,
        initial_task,
        cpu.clone(),
        PathBuf::from("/home/user/mremap_priv_anon"),
        &args,
        &env,
    )
    .unwrap();
    match scheduler.run(None, None) {
        Err(SchedulerError::Stopped(StopReason::ProcessExit)) => Ok(()),
        _ => Err(GenericError::new(GenericErrorKind::Unknown, "Unexpected stop reason")),
    }
}
