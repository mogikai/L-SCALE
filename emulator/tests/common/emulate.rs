use std::path::Path;
use std::time::Duration;

use byteorder::LittleEndian;

use fuzzyemu::emulator::abstractions::emulator::{Scheduler, SchedulerError, StopReason};
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernelTask;
use fuzzyemu::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use fuzzyemu::emulator::system::clock::Clock;
use fuzzyemu::emulator::system::fd_map::FileDescriptorMap;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::hardware::unicorn::UCWrapper;
use fuzzyemu::emulator::system::memory::Memory;
use fuzzyemu::emulator::system::task_manager::TaskManager;
use fuzzyemu::emulator::system::System;

use crate::common::mocks::tcp_system::MockTCPSystem;
use crate::common::TestFailure;

pub fn emulate(
    sys_path: &Path,
    target_path: &Path,
) -> Result<(), TestFailure> {
    let cpu = UCWrapper::new();

    let task_manager = TaskManager::new();
    let filesystem = Filesystem::from_disk(1_073_741_824, sys_path).unwrap();
    let memory = Memory::<LittleEndian>::new();
    let clock = Clock::new(Duration::from_nanos(1662647309301240322));
    let fd_map = FileDescriptorMap::new();
    let tcp = MockTCPSystem::new();
    let system = System::new(cpu.clone(), memory, filesystem, clock, fd_map, task_manager, tcp);

    let initial_task = LinuxKernelTask::new(system.clone(), 1337, 1);

    let args = [];
    let env = ["SHELL=/bin/sh", "LD_BIND_NOW=true"];
    let mut scheduler = LinuxX86Scheduler::new(system, initial_task, target_path.to_path_buf(), &args, &env)?;
    match scheduler.run(None, None) {
        Err(SchedulerError::Stopped(StopReason::ProcessExit(0))) => Ok(()),
        Err(SchedulerError::Stopped(reason)) => Err(TestFailure::ProcessFailure(reason)),
        _ => Err(TestFailure::Unknown),
    }
}
