use std::arch::x86_64::_mm256_undefined_si256;
use std::cell::RefCell;
use std::fmt::Debug;
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;
use std::time::Duration;

use clap::Parser;
use core_affinity::CoreId;
use petgraph::matrix_graph::NodeIndex;
use tracing::debug;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

use mogi_emulator::LittleEndian;
use mogi_emulator::emulator::GenericEmulator;
use mogi_emulator::emulator::interface::emulator::Emulator;
use mogi_emulator::emulator::platform::linux::kernel::LinuxKernelTask;
use mogi_emulator::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use mogi_emulator::emulator::system::System;
use mogi_emulator::emulator::system::clock::Clock;
use mogi_emulator::emulator::system::fd_map::FileDescriptorMap;
use mogi_emulator::emulator::system::filesystem::Filesystem;
use mogi_emulator::emulator::system::hardware::unicorn::UCWrapper;
use mogi_emulator::emulator::system::memory::Memory;
use mogi_emulator::emulator::system::prng::SeedablePrngMap;
use mogi_emulator::emulator::system::stdio::Stdio;
use mogi_emulator::emulator::system::task_manager::TaskManager;
use mogi_fuzzer::core::fuzzer::Fuzzer;
use mogi_fuzzer::io::tcp::TCP;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The path on the host system to the root of the system directory that is used for emulation.
    #[arg(long, default_value = "emulator/systems/ubuntu2310")]
    system_root: String,

    #[arg(long, default_value = "fuzzer/assets/opcua")]
    context: String,

    /// The path on the emulated system to the binary that will be emulated initially.
    #[arg(long, default_value = "/home/user/open62541_server_fuzzing2")]
    binary: String,

    /// If set to true, the entire execution will be traced and a disassembly is printed for each instruction that is executed.
    #[arg(short, long, default_value_t = false)]
    decode: bool,
}

fn main() {
    core_affinity::set_for_current(CoreId { id: 0 });
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("FUZE_LOG"))
        .init();
    let fuzzer_args = Args::parse();
    let cpu = UCWrapper::new();
    let mmu = cpu.clone();
    let task_manager = TaskManager::new();
    let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(&fuzzer_args.system_root)).unwrap();
    let memory = Memory::<LittleEndian>::new();
    let clock = Clock::new(Duration::from_nanos(1662647309301240322));
    let prng_map = SeedablePrngMap::new(0xcafef00dd15ea5e5cafef00dd15ea5e5_u128.to_ne_bytes());
    let file_descriptor_map = FileDescriptorMap::new();
    let tcp = Rc::new(RefCell::new(TCP::new()));
    let stdio = Rc::new(RefCell::new(Stdio::new(true)));
    let system =
        System::new(cpu.clone(), mmu, memory, filesystem, clock, prng_map, file_descriptor_map, task_manager, tcp.clone(), stdio);

    let initial_task = LinuxKernelTask::new(system.clone(), 1337, 1);

    let args = [];
    let env = ["SHELL=/bin/sh", "LD_DEBUG=none", "LD_BIND_NOW=true"];
    let binary_path = PathBuf::from(&fuzzer_args.binary);
    let scheduler = LinuxX86Scheduler::new(
        system.clone(),
        initial_task,
        binary_path.to_owned(),
        &args,
        &env,
        binary_path.parent().unwrap().to_path_buf(),
    )
    .unwrap();

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
    EmulatorT: Emulator + Debug,
{
    let mut fuzzer = Fuzzer::from_serialized_state(
        emulator,
        PathBuf::from(args.context),
        PathBuf::from(".output/fuzzer_state.fst").as_path(),
    );
    fuzzer.trace_path(NodeIndex::from(22));
    debug!("{fuzzer:?}");
}
