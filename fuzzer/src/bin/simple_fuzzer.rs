extern crate core;

use std::cell::RefCell;
use std::collections::{
    HashMap,
    HashSet,
};
use std::fmt::Debug;
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;
use std::time::{
    Duration,
    Instant,
};

use clap::Parser;
use mogi_cpu::cpu::builder::CpuBuilder;
use mogi_cpu::Triple;
use rand::seq::SliceRandom;
use tracing::debug;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    fmt,
    EnvFilter,
};

use mogi_emulator::emulator::abstractions::errors::GenericError;
use mogi_emulator::emulator::abstractions::file_descriptors::FileOperationsReady;
use mogi_emulator::emulator::abstractions::hooks::Hooks;
use mogi_emulator::emulator::abstractions::kernel_task::KernelTask;
use mogi_emulator::emulator::abstractions::network::{
    IPAddress,
    SocketAddress,
};
use mogi_emulator::emulator::abstractions::restore::Restorable;
use mogi_emulator::emulator::interface::emulator::{
    Emulator,
    ExitReason,
};
use mogi_emulator::emulator::interface::network::tcp::{
    TCPSocket,
    TCPSystemControl,
};
use mogi_emulator::emulator::platform::linux::kernel::LinuxKernelTask;
use mogi_emulator::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use mogi_emulator::emulator::system::clock::Clock;
use mogi_emulator::emulator::system::fd_map::FileDescriptorMap;
use mogi_emulator::emulator::system::filesystem::Filesystem;
use mogi_emulator::emulator::system::hardware::unicorn::UCWrapper;
use mogi_emulator::emulator::system::memory::{
    MappingType,
    Memory,
};
use mogi_emulator::emulator::system::prng::SeedablePrngMap;
use mogi_emulator::emulator::system::task_manager::TaskManager;
use mogi_emulator::emulator::system::System;
use mogi_emulator::emulator::GenericEmulator;
use mogi_emulator::LittleEndian;
use mogi_fuzzer::io::tcp::TCP;
use mogi_mem::Mmu;
use mogi_utilities::error::memory::MemoryError;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The path on the host system to the root of the system directory that is used for emulation.
    #[arg(long, default_value = "emulator/systems/ubuntu2310")]
    system_root: String,

    /// The path on the emulated system to the binary that will be emulated initially.
    #[arg(long, default_value = "/home/user/open62541_server_fuzzing_fixed")]
    binary: String,

    /// If set to true, the entire execution will be traced and a disassembly is printed for each instruction that is executed.
    #[arg(short, long, default_value_t = false)]
    decode: bool,
}

fn main() -> Result<(), GenericError> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("FUZE_LOG"))
        .init();
    let mut blocks = HashMap::<(u64, u64, u64), u64>::new();
    let mut prev_block: u64 = 0;
    let mut prev_block2: u64 = 0;
    let mut total_blocks = Rc::new(RefCell::new(0));
    let pargs = Args::parse();

    let duration = {
        // let cpu = UCWrapper::new();
        let mmu = Rc::new(RefCell::new(Mmu::new()));
        let cpu = Rc::new(RefCell::new(
            CpuBuilder::new(mmu.clone())
                .target_triple(Triple::host())
                .build()
                .unwrap(),
        ));

        let task_manager = TaskManager::new();
        let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(&pargs.system_root)).unwrap();
        let memory = Memory::<LittleEndian>::new();
        let clock = Clock::new(Duration::from_nanos(1662647309301240322));
        let prng_map = SeedablePrngMap::new(0xcafef00dd15ea5e5cafef00dd15ea5e5_u128.to_ne_bytes());
        let file_descriptor_map = FileDescriptorMap::new();
        let tcp = Rc::new(RefCell::new(TCP::new()));
        let system = System::new(
            cpu.clone(),
            memory,
            filesystem,
            clock,
            prng_map,
            file_descriptor_map,
            task_manager,
            tcp.clone(),
        );

        // let total_blocks_hook = total_blocks.clone();
        // cpu.borrow_mut()
        //     .add_block_hook(Box::new(move |_, addr, _| {
        //         *total_blocks_hook.borrow_mut() += 1;
        //         // *blocks.entry((prev_block2, prev_block, addr)).or_insert(0) += 1;
        //         // prev_block2 = prev_block;
        //         // prev_block = addr;
        //         // TODO: Is this a performance bottleneck, do we have to use a simpler map?
        //     }))?;

        // TODO: Only one task for now. Need to implement a scheduler that switches tasks.
        let initial_task = LinuxKernelTask::new(system.clone(), 1337, 1);
        let task_id = initial_task.task_id();

        // let mut data = vec![b'A'; 0x01];
        // data.extend(vec![b'x', b'y', b'z', b'x', b'y', b'z', b'\n']);
        // let demo_handle = Box::new(ReadOnly::from(data));
        // kernel.borrow_mut().add_replacement_handle(0, demo_handle);
        let args = [];
        let env = ["SHELL=/bin/sh", "LD_DEBUG=all", "LD_BIND_NOW=false"];
        let scheduler =
            LinuxX86Scheduler::new(system.clone(), initial_task, PathBuf::from(&pargs.binary), &args, &env).unwrap();

        let mut emulator = GenericEmulator::new(scheduler, system.clone());

        if pargs.decode {
            emulator.enable_trace().unwrap();
        }

        system.task_manager.borrow().tasks().try_for_each(|task| {
            if let Some(load_base) = task.kernel().load_base() {
                cpu.borrow_mut().add_breakpoint(load_base + 0xf3fb6)
            } else {
                Ok(())
            }
        })?;
        let start = Instant::now();
        let mut result = emulator.run(None, None);
        debug!("Emulation returned {result:?}");
        if let Ok(ExitReason::Breakpoint) = result {
            debug!("Creating snapshot");
            emulator.take_snapshot()?;
            system.task_manager.borrow().tasks().try_for_each(|task| {
                if let Some(load_base) = task.kernel().load_base() {
                    cpu.borrow_mut().remove_breakpoint(load_base + 0xf3fb6)
                } else {
                    Ok(())
                }
            })?;
            debug!("Trying to resume");
            result = emulator.run(None, None);
            debug!("Emulation returned {result:?}");
        }
        //////////////// DEMO FUZZING ////////////////
        // 100k fuzzing iterations
        let mut active_connections = vec![];
        emulator.take_snapshot()?;
        for _ in 0..10 {
            {
                let mut tcp = tcp.borrow_mut();
                active_connections.retain(|sock| tcp.socket(*sock).is_some());
                if active_connections.is_empty() {
                    let listener_id = *tcp
                        .sockets()
                        .find(|(sid, s)| s.is_listener() && s.interest().contains(FileOperationsReady::Read))
                        .map(|(sid, s)| sid)
                        .unwrap();

                    active_connections.push(
                        tcp.new_connection(
                            &listener_id,
                            SocketAddress::IP(IPAddress::V6 {
                                port: 1337,
                                flowinfo: 0,
                                addr: [0, 0, 0, 0, 0, 0, 0, 123, 0, 0, 0, 0, 0, 0, 0, 0],
                                scope_id: 0,
                            }),
                        )
                        .unwrap(),
                    );

                    active_connections.push(
                        tcp.new_connection(
                            &listener_id,
                            SocketAddress::IP(IPAddress::V6 {
                                port: 1338,
                                flowinfo: 0,
                                addr: [0, 0, 0, 0, 0, 0, 0, 123, 0, 0, 0, 0, 0, 0, 0, 0],
                                scope_id: 0,
                            }),
                        )
                        .unwrap(),
                    );
                }
            }
            let mut tcp = tcp.borrow_mut();
            // let mut data = VecDeque::new();
            // data.push_front(hex::decode("48454c463800000000000000ffff0000ffff00000000000000000000180000006f70632e7463703a2f2f6c6f63616c686f73743a34383430").unwrap());
            // data.push_front(hex::decode("4f504e4684000000000000002f000000687474703a2f2f6f7063666f756e646174696f6e2e6f72672f55412f5365637572697479506f6c696379234e6f6e65ffffffffffffffff01000000010000000100be0100005d738f5c000000000000000000000000ffffffff00000000000000000000000000000001000000ffffffffc0270900").unwrap());

            if let Some(sock) = active_connections.choose(&mut rand::thread_rng()) {
                let socket = tcp.socket_mut(*sock).unwrap();
                socket.push_data(
                    hex::decode("48454c463800000000000000ffff0000ffff00000000000000000000180000006f70632e7463703a2f2f6c6f63616c686f73743a34383430").unwrap().as_slice()
                ).unwrap();
                socket.push_data(hex::decode("4f504e4684000000000000002f000000687474703a2f2f6f7063666f756e646174696f6e2e6f72672f55412f5365637572697479506f6c696379234e6f6e65ffffffffffffffff01000000010000000100be0100005d738f5c000000000000000000000000ffffffff00000000000000000000000000000001000000ffffffffc0270900").unwrap().as_slice()).unwrap();
                // tcp.push_data(
                //     *sock,
                //     (0..rand::thread_rng().gen_range(0..500))
                //         .map(|_| rand::thread_rng().gen::<u8>())
                //         .collect::<Vec<_>>()
                //         .as_slice(),
                // )
                // .unwrap();
            }

            drop(tcp);
            // TODO: only wake up relevant tasks instead of every task.
            system
                .task_manager
                .borrow_mut()
                .tasks_mut()
                .for_each(|t| t.wake_up());
            debug!("Trying to resume");
            result = emulator.run(None, None);
            debug!("Emulation returned {result:?}");
            emulator.restore_snapshot()?;
        }
        //////////////// DEMO FUZZING ////////////////
        let start_iterations = Instant::now();
        // for _ in 0..100 {
        //     if let Err(SchedulerError::NoRunnableTasks) = result {
        //         scheduler.restore()?;
        //         result = scheduler.run(None, None);
        //     }
        // }
        // println!("Iterations per second: {:?}", 100f64 / start_iterations.elapsed().as_secs_f64());
        system
            .memory
            .borrow_mut()
            .mappings(&task_id)
            .try_for_each(|mapping| {
                /*
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
                */
                Ok::<(), MemoryError>(())
            })?;
        start.elapsed()
    };

    debug!("BB/s: {:?}", *total_blocks.borrow() as f64 / duration.as_secs_f64());
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
