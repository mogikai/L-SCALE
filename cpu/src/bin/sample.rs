#![allow(dead_code, unused_variables, unused_mut)]
use clap::Parser;
use mogi_cpu::cpu::CPU;
use mogi_cpu::jit::x86::X86MogiJit;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::SleighPreprocessorFactory;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::lifter::Lifter;
use mogi_sleigh::lifter::cache::LifterCache;
use mogi_sleigh::lifter::source::DirectlyMappedBytes;
use mogi_sleigh::spec::ProcessorSpecification;
use std::cell::RefCell;
use std::env::current_dir;
use std::fs::{
    File,
    OpenOptions,
};
use std::hint::black_box;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, default_value = "0")]
    constructor_id: usize,
}

fn main() {
    let args = Args::parse();
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("FUZE_LOG")
                .from_env_lossy(),
        )
        .init();
    // let test_exe = include_bytes!("./../../../emulator/systems/ubuntu2304/home/user/main");
    let spec_path = Path::new("./deps/Ghidra/Ghidra/Processors/x86/data/languages/x86-64.slaspec");
    // let spec_path = Path::new("./cpu/Ghidra/Ghidra/Processors/x86/data/languages/x86.slaspec");
    // let spec_path = Path::new("./sleigh/ia.sinc");
    let mut compiled_spec_path = current_dir().unwrap();
    compiled_spec_path.push(".output/x86.mcs");
    let mut file = File::open(&compiled_spec_path);
    let specification = match file {
        Ok(mut file) => ProcessorSpecification::load(&mut file),
        Err(_) => {
            let spec_dir = spec_path.parent().unwrap();
            let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
            let mut compiler = SleighCompilerBuilder::new()
                .preprocessor_factory(factory)
                .build();
            let specification = compiler.compile(spec_path).unwrap();
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(compiled_spec_path)
                .unwrap();
            specification.save(&mut file);
            specification
        }
    };
    // println!("{:?}", specification.list_global_varnode_strings());
    println!("{:?}", specification.space_max_used_offset(3.into()).unwrap());
    specification.debug_space_allocation(3.into()).unwrap();
    let source = DirectlyMappedBytes::from(b"\x21\xD8");
    let mut cache = LifterCache::default();
    // mod=1
    let mut context_provider = specification.context_provider();
    // longMode=1
    *context_provider
        .context_value_mut(specification.context_id("contextreg").unwrap())
        .unwrap() = 0x51;
    let mut lifter = Lifter::new(&specification, &mut context_provider, &mut cache, &source);
    lift(&mut lifter);
    // TODO: Disassemble at offset 0x0000000000001060
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    let specification = Rc::new(RefCell::new(specification));
    let mut cpu = CPU::new(X86MogiJit::new(specification.clone(), mmu.clone(), false), specification, mmu);
    // cpu.run().unwrap();
}

fn lift(lifter: &mut Lifter<DirectlyMappedBytes>) {
    let count = 100_000;
    let start = Instant::now();
    for _ in 0..count {
        let (next, inst) = lifter.lift_single(0u64.into()).unwrap();
        black_box(inst);
    }
    let duration = start.elapsed();
    println!("Lifting time: {:?}", duration / count);
}
