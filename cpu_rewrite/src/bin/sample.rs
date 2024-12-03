#![allow(dead_code, unused_variables, unused_mut)]
use clap::Parser;
use mogi_mem::mmu::MogiMMU;
use mogi_neocpu::cpu::CPU;
use mogi_neocpu::jit::x86::X86MogiJit;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::lifter::Lifter;
use mogi_sleigh::lifter::cache::LifterCache;
use mogi_sleigh::lifter::source::DirectlyMappedBytes;
use mogi_sleigh::preprocessor::SleighPreprocessorFactory;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;
use tracing::info;
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
    let test_exe = include_bytes!("./../../../emulator/systems/ubuntu2304/home/user/main");
    let spec_path = Path::new("./cpu/Ghidra/Ghidra/Processors/x86/data/languages/x86-64.slaspec");
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let mut compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let specification = compiler.compile(spec_path).unwrap();
    // println!("{:?}", specification.list_global_varnode_strings());
    println!("{:?}", specification.space_max_used_offset(3.into()).unwrap());
    specification.debug_space_allocation(3.into()).unwrap();
    let source = DirectlyMappedBytes::from(b"\x83\xc0\x01");
    let mut cache = LifterCache::default();
    let mut lifter = Lifter::new(&specification, &mut cache, &source);
    let (next, inst) = lifter.lift(0u64.into(), args.constructor_id).unwrap();
    // TODO: Disassemble at offset 0x0000000000001060
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    let specification = Rc::new(RefCell::new(specification));
    let mut cpu = CPU::new(X86MogiJit::new(specification.clone(), mmu.clone(), true), specification, mmu);
    cpu.run().unwrap();
}
