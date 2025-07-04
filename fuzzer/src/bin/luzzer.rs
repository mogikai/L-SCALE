use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Range;
use std::option::Option;
use std::path::{
    Path,
    PathBuf,
};
use std::process::{
    Child,
    Command,
    exit,
};
use std::rc::Rc;
use std::sync::{
    Arc,
    Mutex,
};
use std::thread;
use std::thread::sleep;
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
use itertools::Itertools;
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
use strum::IntoEnumIterator;
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
    Evaluate(Evaluate),
    Completions {
        #[arg(long)]
        shell: Shell,
    },
}

#[derive(Args, Debug)]
struct GeneralLearnerArgs {
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

    /// If set, terminate the execution after the given number of seconds
    #[arg(long)]
    timeout: Option<u64>,
}

#[derive(Args, Debug)]
struct Evaluate {
    #[command(flatten)]
    general: GeneralLearnerArgs,

    #[arg(long)]
    threshold_start: u64,

    #[arg(long)]
    threshold_end: u64,
}

#[derive(Args, Debug)]
struct Learn {
    #[command(flatten)]
    general: GeneralLearnerArgs,

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
    match Cli::parse().command {
        Commands::Learn(learn_args) => learn(learn_args),
        Commands::Evaluate(eval_args) => evaluate(eval_args),
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            generate(shell, &mut command, name, &mut std::io::stdout());
        }
    };
}

fn evaluate(args: Evaluate) {
    let mut args_vec = vec![];
    args_vec.extend([
        format!("--context={}", args.general.context.display()),
        format!("--system-root={}", args.general.system_root.display()),
        format!("--binary={}", args.general.binary.display()),
        format!("--output={}", args.general.output.display()),
        format!("--corpus={}", args.general.corpus),
    ]);
    if let Some(timeout) = args.general.timeout {
        args_vec.push(format!("--timeout={}", timeout));
    }

    let cores = core_affinity::get_core_ids().unwrap_or_else(|| {
        thread::available_parallelism()
            .map(|n| (0..n.get()).map(|id| CoreId { id }).collect())
            .unwrap_or_else(|_| vec![CoreId { id: 0 }])
    });
    let mut cores = Arc::new(Mutex::new(
        cores
            .into_iter()
            .map(|core| (core, None::<Child>))
            .collect::<HashMap<_, _>>(),
    ));

    let mut commands = Vec::new();

    let combinations = BucketingMode::iter()
        .cartesian_product([true, false].into_iter())
        .cartesian_product(args.threshold_start..=args.threshold_end);
    for ((mode, sparse), threshold) in combinations {
        let mut args_vec = args_vec.clone();
        args_vec.push(format!("--bucketing-mode={}", mode.to_string().to_lowercase()));
        if sparse {
            args_vec.push("--sparse-behavior".to_string());
        }
        args_vec.push(format!("--threshold={}", threshold));
        let mut cmd = Command::new("./luzzer");
        cmd.arg("learn").args(args_vec);
        commands.push(cmd);
    }

    let cores_clone = cores.clone();
    ctrlc::set_handler(move || {
        let mut cores = cores_clone.lock().unwrap();
        cores.iter_mut().for_each(|(_, child)| {
            if let Some(child) = child {
                child.kill().unwrap();
            }
        });
        exit(0);
    })
    .unwrap();

    for mut command in commands {
        // Find a free core to spawn a command
        let core = cores
            .lock()
            .unwrap()
            .iter_mut()
            .find_map(|(core, child)| {
                if let Some(child) = child {
                    if let Ok(result) = child.try_wait() {
                        if result.is_some() { Some(core) } else { None }
                    } else {
                        None
                    }
                } else {
                    Some(core)
                }
            })
            .copied();
        let Some(core) = core else {
            // If no core is available we just wait for a second and try again.
            sleep(Duration::from_secs(100));
            continue;
        };

        command.arg(format!("--pin-core={}", core.id));
        // println!("Spawning command {:?}", command);
        let child = command.spawn().unwrap();
        println!("Spawned command on core {:?}", core);
        cores.lock().unwrap().insert(core, Some(child));
    }

    // Wait for all remaining tasks
    for (_, child) in cores.lock().unwrap().iter_mut() {
        if let Some(child) = child {
            child.wait().unwrap();
        }
    }
}

fn learn(luzzer_args: Learn) {
    if let Some(id) = luzzer_args.pin_core {
        core_affinity::set_for_current(CoreId { id });
    }
    let cpu = UCWrapper::new();
    let mmu = cpu.clone();
    let task_manager = TaskManager::new();
    let filesystem = Filesystem::from_disk(1_073_741_824, Path::new(&luzzer_args.general.system_root)).unwrap();
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
    let binary_path = PathBuf::from(&luzzer_args.general.binary);
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

    if let Some(timeout) = luzzer_args.general.timeout {
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
        .context_path(args.general.context)
        .corpus(args.general.corpus)
        .maybe_do_refinement(args.do_refinement)
        .threshold(args.threshold.into())
        .output_path(args.general.output)
        .detailed_graph_building(args.detailed_graph_building)
        .fuzzy_learning(args.fuzzy_learning)
        .maybe_backtracking_chance(args.backtracking_chance)
        .bucketing_mode(args.bucketing_mode)
        .sparse_behavior(args.sparse_behavior)
        .executable_path(args.general.binary);
    let mut luzzer = Luzzer::new(emulator, config.build());
    luzzer.run();
}
