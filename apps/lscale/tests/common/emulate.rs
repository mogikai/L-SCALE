use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use crate::common::TestFailure;
use crate::common::mocks::tcp_system::MockTCPSystem;
use mogi_emulator::LittleEndian;
use mogi_emulator::emulator::abstractions::emulator::Scheduler;
use mogi_emulator::emulator::platform::linux::kernel::LinuxKernelTask;
use mogi_emulator::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use mogi_emulator::emulator::system::System;
use mogi_emulator::emulator::system::clock::Clock;
use mogi_emulator::emulator::system::fd_map::FileDescriptorMap;
use mogi_emulator::emulator::system::filesystem::Filesystem;
use mogi_emulator::emulator::system::memory::Memory;
use mogi_emulator::emulator::system::prng::SeedablePrngMap;
use mogi_emulator::emulator::system::stdio::Stdio;
use mogi_emulator::emulator::system::task_manager::TaskManager;
use mogi_system::errors::SchedulerError;
use mogi_system::errors::cpu::StopReason;
use mogi_unicorn::UCWrapper;

pub fn emulate(
    sys_path: &Path,
    target_path: &Path,
) -> Result<(), TestFailure> {
    // TODO: Adapt this to new emulation architecture once finished
    let cpu = UCWrapper::new();
    let mmu = cpu.clone();
    let task_manager = TaskManager::new();
    let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(sys_path)).unwrap();
    let memory = Memory::<LittleEndian>::new();
    let clock = Clock::new(Duration::from_nanos(1662647309301240322));
    let prng_map = SeedablePrngMap::new(0xcafef00dd15ea5e5cafef00dd15ea5e5_u128.to_ne_bytes());
    let fd_map = FileDescriptorMap::new();
    let tcp = Rc::new(RefCell::new(MockTCPSystem::new()));
    let stdio = Rc::new(RefCell::new(Stdio::new(true)));
    let system = System::new(cpu, mmu, memory, filesystem, clock, prng_map, fd_map, task_manager, tcp, stdio);

    let initial_task = LinuxKernelTask::new(system.clone(), 1337, 1);

    let args = [];
    let env = ["SHELL=/bin/sh", "LD_BIND_NOW=true"];
    let mut scheduler = LinuxX86Scheduler::new(
        system.clone(),
        initial_task,
        target_path.to_path_buf(),
        &args,
        &env,
        target_path.parent().unwrap().to_path_buf(),
    )?;
    match scheduler.run(None, None) {
        Err(SchedulerError::Stopped(StopReason::ProcessExit(0))) => Ok(()),
        Err(SchedulerError::Stopped(reason)) => Err(TestFailure::ProcessFailure(reason)),
        _ => Err(TestFailure::Unknown),
    }
}
