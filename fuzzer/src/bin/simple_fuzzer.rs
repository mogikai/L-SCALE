extern crate core;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Parser;
use log::debug;

use fuzzyemu::emulator::abstractions::cpu::CPU;
use fuzzyemu::emulator::abstractions::emulator::StopReason::Breakpoint;
use fuzzyemu::emulator::abstractions::emulator::{Scheduler, SchedulerError};
use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::hooks::Hooks;
use fuzzyemu::emulator::abstractions::mmu::MemoryError;
use fuzzyemu::emulator::abstractions::restore::Restorable;
use fuzzyemu::emulator::platform::linux::architecture::x86::X86;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernelTask;
use fuzzyemu::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use fuzzyemu::emulator::system::clock::Clock;
use fuzzyemu::emulator::system::fd_map::FileDescriptorMap;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::hardware::unicorn::UCWrapper;
use fuzzyemu::emulator::system::memory::{MappingType, Memory};
use fuzzyemu::emulator::system::System;
use fuzzyemu::LittleEndian;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The path on the host system to the root of the system directory that is used for emulation.
    #[arg(long, default_value = "emulator/systems/ubuntu2304")]
    system_root: String,

    /// The path on the emulated system to the binary that will be emulated initially.
    #[arg(long, default_value = "/home/user/open62541_server2")]
    binary: String,

    /// If set to true, the entire execution will be traced and a disassembly is printed for each instruction that is executed.
    #[arg(short, long, default_value_t = false)]
    decode: bool,
}

fn main() -> Result<(), GenericError> {
    pretty_env_logger::init();
    let mut blocks = HashMap::<(u64, u64, u64), u64>::new();
    let mut prev_block: u64 = 0;
    let mut prev_block2: u64 = 0;
    let mut total_blocks: u64 = 0;
    let pargs = Args::parse();

    let duration = {
        let cpu = UCWrapper::new();

        let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(&pargs.system_root)).unwrap();
        let memory = Memory::new();
        let clock = Clock::new(Duration::from_nanos(1662647309301240322));
        let file_descriptor_map = FileDescriptorMap::new();
        let system = System::new(memory, filesystem, clock, file_descriptor_map);

        cpu.borrow_mut().add_block_hook(Box::new(|_, addr, _| {
            total_blocks += 1;
            // *blocks.entry((prev_block2, prev_block, addr)).or_insert(0) += 1;
            // prev_block2 = prev_block;
            // prev_block = addr;
            // TODO: Is this a performance bottleneck, do we have to use a simpler map?
        }))?;

        // TODO: Only one task for now. Need to implement a scheduler that switches tasks.
        let initial_task = LinuxKernelTask::<LittleEndian, X86>::new(system.clone(), 1337, 1);
        let task_id = initial_task.task_id();

        // let mut data = vec![b'A'; 0x01];
        // data.extend(vec![b'x', b'y', b'z', b'x', b'y', b'z', b'\n']);
        // let demo_handle = Box::new(ReadOnly::from(data));
        // kernel.borrow_mut().add_replacement_handle(0, demo_handle);
        let args = [];
        let env = ["SHELL=/bin/sh", "LD_DEBUG=all", "LD_BIND_NOW=false"];
        let mut scheduler = LinuxX86Scheduler::new(
            system.clone(),
            initial_task,
            cpu.clone(),
            PathBuf::from(&pargs.binary),
            &args,
            &env,
        )
        .unwrap();

        if pargs.decode {
            scheduler.enable_trace().unwrap();
        }

        scheduler.try_for_each_task(|task| {
            if let Some(load_base) = task.load_base() {
                cpu.borrow_mut().add_breakpoint(load_base + 0xf3fb6)
            } else {
                Ok(())
            }
        })?;
        let start = Instant::now();
        let mut result = scheduler.run(None, None);
        debug!("Emulation returned {result:?}");
        if let Err(SchedulerError::Stopped(Breakpoint)) = result {
            debug!("Creating snapshot");
            scheduler.snapshot()?;
            scheduler.try_for_each_task(|task| {
                if let Some(load_base) = task.load_base() {
                    cpu.borrow_mut().remove_breakpoint(load_base + 0xf3fb6)
                } else {
                    Ok(())
                }
            })?;
            debug!("Trying to resume");
            result = scheduler.run(None, None);
            debug!("Emulation returned {result:?}");
        }
        let start_iterations = Instant::now();
        for _ in 0..100 {
            if let Err(SchedulerError::NoRunnableTasks) = result {
                scheduler.restore()?;
                result = scheduler.run(None, None);
            }
        }
        println!("Iterations per second: {:?}", 100f64 / start_iterations.elapsed().as_secs_f64());
        system
            .memory
            .borrow_mut()
            .iterator_mappings(&task_id, |iter| {
                for mapping in iter {
                    let path = if let MappingType::PrivateFile(ref ifd) = mapping.mapping_type {
                        let fd_map = system.fd_map.borrow_mut();
                        let file = fd_map.get_internal(&task_id, ifd).unwrap();
                        let path = file
                            .data
                            .file
                            .as_ref()
                            .unwrap()
                            .get_path(&system.filesystem.borrow_mut())
                            .map_err(|_| MemoryError::Unknown)?;
                        Some(path)
                    } else {
                        None
                    };
                    debug!("Mapping: {} File: {:?}", mapping, path);
                }
                Ok::<(), MemoryError>(())
            })?;
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
    debug!("Total 3-edges executed: {:?}", blocks.values().cloned().reduce(|acc, b| acc + b));
    Ok(())
}
