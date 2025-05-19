#![feature(test)]

use std::cell::RefCell;
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;
use std::sync::{
    LazyLock,
    RwLock,
};

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use mogi_cpu::cpu::MogiCPU;
use mogi_cpu::jit::api::{JITTaskRuntimeContext, MogiJit};
use mogi_cpu::jit::x86::X86MogiJit;
use mogi_cpu::new_cpu_trait::{
    CPU,
    RunLimit,
};
use mogi_mem::PAGE_SIZE;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::SleighPreprocessorFactory;
use mogi_sleigh::compiler::SleighCompilerResult;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_system::components::mmu::{
    MMU,
    MemoryAction,
    Permission,
};
use zerocopy::IntoBytes;

static SPEC: LazyLock<RwLock<ProcessorSpecification>> = LazyLock::new(|| {
    let mut spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_path.push("resources/test/x86/x86-64.slaspec");
    RwLock::new(build(&spec_path).unwrap())
});

fn build(spec_path: &Path) -> SleighCompilerResult<ProcessorSpecification> {
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    compiler.compile(spec_path)
}

pub(crate) fn setup_with_bytes(bytes: &[u8]) -> (MogiCPU<X86MogiJit, MogiMMU>, Rc<RefCell<MogiMMU>>) {
    let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));
    let stack_page_count = 8;
    let page_count = (bytes.len() + PAGE_SIZE - 1) / PAGE_SIZE;
    let stack_addr = 0xFFFF_FFFF_FFFF_0000u64;

    // Set up the mmu.
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    {
        let mut mmu = mmu.borrow_mut();
        // We allocate enough memory
        mmu.alloc_physical(0x0u64.into(), (page_count + stack_page_count) * PAGE_SIZE)
            .unwrap();

        // And identity-map the virtual addresses
        for page_id in 0..page_count {
            mmu.set_page_table_entry(
                (page_id as u64 * 4096u64).into(),
                (page_id as u64 * 4096u64).into(),
                Permission::ALL,
            )
            .unwrap();
        }
        // Map stack pages downwards.
        for page_id in 0..stack_page_count {
            mmu.set_page_table_entry(
                (stack_addr - (page_id as u64 * 4096u64)).into(),
                ((page_id + page_count) as u64 * 4096u64).into(),
                Permission::ALL,
            )
            .unwrap();
        }

        // Map dead address
        // TODO Permission currently sets physical page permission which is wrong
        mmu.set_page_table_entry(0xFFFF_FFFF_D34D_0000u64.into(), 0x0u64.into(), Permission::ALL)
            .unwrap();

        // And then write the instruction to the allocated memory.
        mmu.write_bytes(0x0u64.into(), &bytes[..]).unwrap();
    }

    // Set up the just in time compiler. TODO: Maybe we can refactor this such that the jit doesn't need the spec.
    let mut jit =
        X86MogiJit::new(spec.clone(), mmu.clone(), std::env::var("MOGI_DEBUG_JIT").map_or(false, |val| val == "true"));

    let mmu_tlb = mmu.clone();
    jit.register_tlb_read_miss_callback(move |ctx, addr, size, space| {
        let mmu = mmu_tlb.borrow();
        let tlb = ctx.tlb_mut();
        unsafe {
            mmu.populate_tlb(tlb, addr, MemoryAction::READ)
                .map_err(|e| todo!())
        }
    })
    .unwrap();

    let mmu_tlb = mmu.clone();
    jit.register_tlb_write_miss_callback(move |ctx, addr, size, space| {
        let mmu = mmu_tlb.borrow();
        let tlb = ctx.tlb_mut();
        unsafe {
            mmu.populate_tlb(tlb, addr, MemoryAction::WRITE)
                .map_err(|e| todo!())
        }
    })
    .unwrap();

    // Set up the cpu emulator
    let mut cpu = MogiCPU::new(jit, spec, mmu.clone());

    // Set the instruction pointer to the address we want to start executing at.
    *cpu.register_mut("RIP").unwrap() = 0x0u64;
    *cpu.register_mut("RSP").unwrap() = stack_addr;

    (cpu, mmu)
}

pub fn compile_benchmark(c: &mut Criterion) {
    let mut misc_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o0_path.push("resources/test/executables/misc_o0.o");

    let instr = std::fs::read(&misc_o0_path).unwrap();
    let (mut cpu, mmu) = setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_1000u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // write input to memory
    let numbers_addr = 0xFFFF_FFFF_FF00_0000u64;
    const NUMBERS: [i32; 32] = [
        -13935, 17471, -23649, 20073, 18135, 11677, 28681, 7075, -31766, 27292, 32352, -13904, 8405, -1462, -10314,
        -19981, 692, 22985, -7727, -28196, -11245, -30707, 31868, 20361, -1579, 28034, -22596, 23481, 20717, -6840,
        15357, 7076,
    ];

    mmu.borrow_mut()
        .alloc_physical(numbers_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(numbers_addr.into(), numbers_addr.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut()
        .write(numbers_addr.into(), &NUMBERS)
        .unwrap();

    let mut bytes_sorted = NUMBERS.clone();
    bytes_sorted.sort();
    let mut output = vec![0i32; NUMBERS.len()];

    c.bench_function("compile quicksort", |b| {
        b.iter(|| {
            cpu.flush();

            mmu.borrow_mut()
                .write(numbers_addr.into(), &NUMBERS)
                .unwrap();

            *cpu.register_mut("RIP").unwrap() = 0x1400u64;
            *cpu.register_mut("RSP").unwrap() = stack_addr;
            *cpu.register_mut("RDI").unwrap() = numbers_addr;
            *cpu.register_mut("RSI").unwrap() = 0u64;
            *cpu.register_mut("RDX").unwrap() = NUMBERS.len() as u64 - 1;
            cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
                .unwrap();
        });
    });
}

criterion_group!(benches, compile_benchmark);
criterion_main!(benches);
