use std::cell::RefCell;
use std::fmt::Debug;
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;
use std::time::Duration;

use clap::Parser;
use libafl::corpus::{
    InMemoryCorpus,
    OnDiskCorpus,
};
use libafl::events::SimpleEventManager;
use libafl::feedbacks::CrashFeedback;
use libafl::mutators::havoc_mutations_no_crossover;
use libafl::prelude::{
    QueueScheduler,
    SimpleMonitor,
};
use libafl::stages::mutational::StdMutationalStage;
use libafl::{
    Evaluator,
    Fuzzer,
    StdFuzzer,
};
use libafl_bolts::current_nanos;
use libafl_bolts::rands::StdRand;
use libafl_bolts::tuples::tuple_list;

use fuzzyemu::emulator::abstractions::emulator::Scheduler;
use fuzzyemu::emulator::abstractions::system::SystemControl;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernelTask;
use fuzzyemu::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use fuzzyemu::emulator::system::clock::Clock;
use fuzzyemu::emulator::system::fd_map::FileDescriptorMap;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::hardware::unicorn::UCWrapper;
use fuzzyemu::emulator::system::memory::Memory;
use fuzzyemu::emulator::system::task_manager::TaskManager;
use fuzzyemu::emulator::system::System;
use fuzzyemu::LittleEndian;
use fuzzyfuzz::executor::emulator::EmulatorExecutor;
use fuzzyfuzz::feedback::edge::EdgeFeedback;
use fuzzyfuzz::inputs::emulator::EmulatorInput;
use fuzzyfuzz::io::tcp::TCP;
use fuzzyfuzz::mutators::multi_stream::MultiStreamMutator;
use fuzzyfuzz::observers::emulator::edge::EdgeObserver;
use fuzzyfuzz::observers::emulator::tcp::TCPStreamObserver;
use fuzzyfuzz::state::FUZEState;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The path on the host system to the root of the system directory that is used for emulation.
    #[arg(long, default_value = "emulator/systems/ubuntu2304")]
    system_root: String,

    /// The path on the emulated system to the binary that will be emulated initially.
    #[arg(long, default_value = "/home/user/open62541_server_release")]
    binary: String,

    /// If set to true, the entire execution will be traced and a disassembly is printed for each instruction that is executed.
    #[arg(short, long, default_value_t = false)]
    decode: bool,
}

fn main() {
    let pargs = Args::parse();
    let cpu = UCWrapper::new();
    let task_manager = TaskManager::new();
    let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(&pargs.system_root)).unwrap();
    let memory = Memory::<LittleEndian>::new();
    let clock = Clock::new(Duration::from_nanos(1662647309301240322));
    let file_descriptor_map = FileDescriptorMap::new();
    let tcp = Rc::new(RefCell::new(TCP::new()));
    let system = System::new(cpu.clone(), memory, filesystem, clock, file_descriptor_map, task_manager, tcp.clone());

    let initial_task = LinuxKernelTask::new(system.clone(), 1337, 1);

    let args = [];
    let env = ["SHELL=/bin/sh", "LD_DEBUG=all", "LD_BIND_NOW=true"];
    let mut scheduler =
        LinuxX86Scheduler::new(system.clone(), initial_task, PathBuf::from(&pargs.binary), &args, &env).unwrap();

    if pargs.decode {
        scheduler.enable_trace().unwrap();
    }

    setup_fuzzer(scheduler, system);
}

fn setup_fuzzer<SchedulerT: Scheduler + Debug, SystemT: SystemControl + Debug>(
    emu_scheduler: SchedulerT,
    system: SystemT,
) {
    // let ui = TuiUI::with_version("FUZE".to_string(), "0.0.1".to_string(), true);
    // let mon = TuiMonitor::new(ui);
    let mon = SimpleMonitor::new(|s| println!("{s}"));

    let observer = EdgeObserver::new(system.cpu());
    let tcp_observer = TCPStreamObserver::new(system.tcp());

    let mut feedback = EdgeFeedback::new(&observer);

    let mut objective = CrashFeedback::new();

    let mut state = FUZEState::new(
        StdRand::with_seed(current_nanos()),
        InMemoryCorpus::new(),
        OnDiskCorpus::new(PathBuf::from("./crashes")).unwrap(),
        &mut feedback,
        &mut objective,
    )
    .unwrap();

    let mut mgr = SimpleEventManager::new(mon);
    let scheduler = QueueScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = EmulatorExecutor::new(tuple_list!(observer, tcp_observer), &state, emu_scheduler, system);

    // let mut generator = RandPrintablesGenerator::new(32);

    let mutator = MultiStreamMutator::new(havoc_mutations_no_crossover());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    fuzzer
        .add_input(&mut state, &mut executor, &mut mgr, EmulatorInput::new())
        .unwrap();

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .unwrap();
}
