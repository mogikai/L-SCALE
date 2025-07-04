use std::cell::RefCell;
use std::fmt::Debug;
use std::path::{
    Path,
    PathBuf,
};
use std::process::exit;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use clap::{
    Args,
    CommandFactory,
    Parser,
    Subcommand,
};
use clap_complete::{
    Shell,
    generate,
};
use core_affinity::CoreId;
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
use mogi_fuzzer::core::luzzer::{
    BucketingMode,
    Luzzer,
    LuzzerConfig,
};
use mogi_fuzzer::io::tcp::TCP;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None, name = "luzzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the learner
    Learn(Learn),
    Completions {
        #[arg(long)]
        shell: Shell,
    },
}

#[derive(Args, Debug)]
struct Learn {
    /// The path on the host system to the root of the system directory that is used for emulation.
    #[arg(long)]
    system_root: PathBuf,

    /// The path to the context folder where the corpus lives
    #[arg(long)]
    context: PathBuf,

    /// The name of the corpus to use. The corpus is located inside the <context>/corpus directory.
    #[arg(long)]
    corpus: String,

    /// The path where the state learner puts its output files
    #[arg(long)]
    output: PathBuf,

    /// The path on the emulated system to the binary that will be emulated initially.
    #[arg(long)]
    binary: PathBuf,

    /// Do refinement rounds for the number of times provided.
    #[arg(long)]
    do_refinement: Option<usize>,

    /// If set to true, the entire execution will be traced and a disassembly is printed for each instruction that is executed.
    #[arg(long, default_value_t = false)]
    decode: bool,

    /// Pin the execution of the learner to the provided core number.
    #[arg(long)]
    pin_core: Option<usize>,

    /// Perform detailed graph building. I.e., check closed and consistent properties after each gathering step
    /// and dump the resulting statemachine. This is a lot less efficient and should only be used for demonstration
    /// or debugging purposes.
    #[arg(short, long, default_value_t = false)]
    detailed_graph_building: bool,

    /// The threshold for the observation equality check. Observations are considered equal if their difference is less than the threshold.
    #[arg(short, long)]
    threshold: u64,

    /// Enable fuzzy learning. This leaves out observations based on a threshold value and can be faster but is less
    /// accurate.
    #[arg(long, default_value_t = false)]
    fuzzy_learning: bool,

    /// The chance to backtrack when doing random walk through the prefix set. Must be between 0 and 1
    #[arg(long)]
    backtracking_chance: Option<f64>,

    /// If set, terminate the execution after the given number of seconds
    #[arg(long)]
    timeout: Option<u64>,

    /// The mode to use for observation bucketing.
    /// If using Closest, then for two observations A, B where the threshold area overlaps, for an Observation C
    /// the closest one is chosen as representative.
    /// Example: |----A---|C--B----|
    /// If using Cutoff, then the observation threshold is cut off on the left for overlapping threshold areas.
    /// Example: |----A---C-|-B----|
    #[arg(long)]
    bucketing_mode: BucketingMode,

    /// If provided, the coverage collection mechanism will only set the first address of each basic block
    /// as visited. This can be more efficient but is less accurate than setting every address visited.
    /// In some cases it might also be desirable to have less accurate measurements to tackle the state
    /// explosion problem.
    #[arg(long, default_value_t = false)]
    sparse_behavior: bool,
}

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("MOGI_LOG"))
        .init();
    let luzzer_args = match Cli::parse().command {
        Commands::Learn(learn_args) => learn_args,
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            generate(shell, &mut command, name, &mut std::io::stdout());
            exit(0);
        }
    };
    if let Some(id) = luzzer_args.pin_core {
        core_affinity::set_for_current(CoreId { id });
    }
    let cpu = UCWrapper::new();
    let mmu = cpu.clone();
    let task_manager = TaskManager::new();
    let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(&luzzer_args.system_root)).unwrap();
    let memory = Memory::<LittleEndian>::new();
    let clock = Clock::new(Duration::from_nanos(1662647309301240322));
    let prng_map = SeedablePrngMap::new(0xcafef00dd15ea5e5cafef00dd15ea5e5_u128.to_ne_bytes());
    let file_descriptor_map = FileDescriptorMap::new();
    let tcp = Rc::new(RefCell::new(TCP::new()));
    let stdio = Rc::new(RefCell::new(Stdio::new(true)));
    let system = System::new(
        cpu,
        mmu,
        memory,
        filesystem,
        clock,
        prng_map,
        file_descriptor_map,
        task_manager,
        tcp.clone(),
        stdio,
    );

    let initial_task = LinuxKernelTask::new(system.clone(), 1337, 1);

    let args = [];
    let env = ["SHELL=/bin/sh", "LD_DEBUG=none", "LD_BIND_NOW=true"];
    let binary_path = PathBuf::from(&luzzer_args.binary);
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

    if luzzer_args.decode {
        emulator.enable_trace().unwrap();
    }

    if let Some(timeout) = luzzer_args.timeout {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(timeout));
            tracing::info!("Timeout reached, terminating execution.");
            std::process::exit(0);
        });
    }

    setup_luzzer(luzzer_args, emulator);
}

fn setup_luzzer<EmulatorT>(
    args: Learn,
    emulator: EmulatorT,
) where
    EmulatorT: Emulator,
{
    let config = LuzzerConfig::builder()
        .context_path(args.context)
        .corpus(args.corpus)
        .maybe_do_refinement(args.do_refinement)
        .threshold(args.threshold.into())
        .output_path(args.output)
        .detailed_graph_building(args.detailed_graph_building)
        .fuzzy_learning(args.fuzzy_learning)
        .maybe_backtracking_chance(args.backtracking_chance)
        .bucketing_mode(args.bucketing_mode)
        .sparse_behavior(args.sparse_behavior)
        .executable_path(args.binary);
    let mut luzzer = Luzzer::new(emulator, config.build());
    luzzer.run();
}
