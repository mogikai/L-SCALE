use std::cell::RefCell;
use std::fmt::Debug;
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;
use std::time::Duration;

use clap::Parser;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    fmt,
    EnvFilter,
};

use fuzzyemu::emulator::interface::emulator::Emulator;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernelTask;
use fuzzyemu::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use fuzzyemu::emulator::system::clock::Clock;
use fuzzyemu::emulator::system::fd_map::FileDescriptorMap;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::hardware::unicorn::UCWrapper;
use fuzzyemu::emulator::system::memory::Memory;
use fuzzyemu::emulator::system::task_manager::TaskManager;
use fuzzyemu::emulator::system::System;
use fuzzyemu::emulator::GenericEmulator;
use fuzzyemu::LittleEndian;
use fuzzyfuzz::core::fuzzer::Fuzzer;
use fuzzyfuzz::io::tcp::TCP;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The path on the host system to the root of the system directory that is used for emulation.
    #[arg(long, default_value = "emulator/systems/ubuntu2304")]
    system_root: String,

    #[arg(long, default_value = "fuzzer/assets/opcua")]
    context: String,

    /// The path on the emulated system to the binary that will be emulated initially.
    #[arg(long, default_value = "/home/user/open62541_server_release")]
    binary: String,

    /// If set to true, the entire execution will be traced and a disassembly is printed for each instruction that is executed.
    #[arg(short, long, default_value_t = false)]
    decode: bool,
}

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("FUZE_LOG"))
        .init();
    let fuzzer_args = Args::parse();
    let cpu = UCWrapper::new();
    let task_manager = TaskManager::new();
    let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(&fuzzer_args.system_root)).unwrap();
    let memory = Memory::<LittleEndian>::new();
    let clock = Clock::new(Duration::from_nanos(1662647309301240322));
    let file_descriptor_map = FileDescriptorMap::new();
    let tcp = Rc::new(RefCell::new(TCP::new()));
    let system = System::new(cpu.clone(), memory, filesystem, clock, file_descriptor_map, task_manager, tcp.clone());

    let initial_task = LinuxKernelTask::new(system.clone(), 1337, 1);

    let args = [];
    let env = ["SHELL=/bin/sh", "LD_DEBUG=all", "LD_BIND_NOW=true"];
    let scheduler =
        LinuxX86Scheduler::new(system.clone(), initial_task, PathBuf::from(&fuzzer_args.binary), &args, &env).unwrap();

    let mut emulator = GenericEmulator::new(scheduler, system);

    if fuzzer_args.decode {
        emulator.enable_trace().unwrap();
    }

    setup_fuzzer(fuzzer_args, emulator);
}

fn setup_fuzzer<EmulatorT>(
    args: Args,
    emulator: EmulatorT,
) where
    EmulatorT: Emulator,
{
    let mut fuzzer = Fuzzer::new(emulator, PathBuf::from(args.context));
    fuzzer.run();
}
