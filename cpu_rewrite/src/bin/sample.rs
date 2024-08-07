use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use tracing::debug;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    fmt,
    EnvFilter,
};

use mogi_mem::mmu::MogiMMU;
use mogi_neocpu::cpu::CPU;
use mogi_neocpu::jit::x86::X86MogiJit;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::preprocessor::SleighPreprocessorFactory;

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("FUZE_LOG"))
        .init();
    let test_exe = include_bytes!("./../../../emulator/systems/ubuntu2304/home/user/main");
    let spec_path = Path::new("/home/mark/projects/fraunhofer/fuzzing/prototypes/fuzzyemu/cpu/Ghidra/Ghidra/Processors/x86/data/languages/x86-64.slaspec");
    // let spec_path = Path::new("/home/mark/projects/fraunhofer/fuzzing/prototypes/fuzzyemu/cpu/Ghidra/Ghidra/Processors/x86/data/languages/ia.sinc");
    // let spec_path = Path::new("/home/bas39150/fuzzyemu/sleigh/ia.sinc");
    // let spec_path = Path::new("/home/bas39150/fuzzyemu/Ghidra/Processors/x86/sleigh/ia.sinc");
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let mut compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let specification = compiler.compile(spec_path).unwrap();
    // TODO: Disassemble at offset 0x0000000000001060
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    let mut cpu = CPU::new(X86MogiJit::new(mmu.clone()), mmu);
    // cpu.run().unwrap();
}
