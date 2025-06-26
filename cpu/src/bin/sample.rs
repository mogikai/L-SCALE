use mogi_cpu::cpu::MogiCPU;
use mogi_cpu::jit::api::{
    JITTaskRuntimeContext,
    MogiJit,
};
use mogi_cpu::jit::x86::X86MogiJit;
use mogi_cpu::new_cpu_trait::{
    CPU,
    RunLimit,
};
use mogi_mem::PAGE_SIZE;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::SleighPreprocessorFactory;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_system::components::mmu::{
    MMU,
    MemoryAction,
    Permission,
};
use std::cell::RefCell;
use std::env::current_dir;
use std::fs::{
    File,
    OpenOptions,
};
use std::path::{
    Path,
    PathBuf,
};
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

fn setup_with_bytes(
    bytes: &[u8],
    rip: u64,
) -> (MogiCPU<X86MogiJit, MogiMMU>, Rc<RefCell<MogiMMU>>, Rc<RefCell<ProcessorSpecification>>) {
    let spec_path = Path::new("./deps/Ghidra/Ghidra/Processors/x86/data/languages/x86-64.slaspec");
    let spec = load_specification(spec_path);
    let stack_page_count = 8;
    let page_count = (bytes.len() + PAGE_SIZE - 1) / PAGE_SIZE;
    let stack_addr = 0xFFFF_FFFF_FFFF_0000u64;

    // Set up the mmu.
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    {
        let mut mmu = mmu.borrow_mut();

        // Allocate instructions
        mmu.alloc_physical(0x0u64.into(), page_count * PAGE_SIZE)
            .unwrap();

        // Map instructions
        for page_id in 0..page_count {
            mmu.set_page_table_entry(
                (page_id as u64 * 4096u64).into(),
                (page_id as u64 * 4096u64).into(),
                Permission::ALL,
            )
            .unwrap();
        }

        // Allocate stack
        mmu.alloc_physical(
            (stack_addr - (stack_page_count - 1) * PAGE_SIZE as u64).into(),
            stack_page_count as usize * PAGE_SIZE,
        )
        .unwrap();

        // Map stack pages downwards.
        for page_id in 0..stack_page_count {
            mmu.set_page_table_entry(
                (stack_addr - (page_id as u64 * 4096u64)).into(),
                (stack_addr - (page_id as u64 * 4096u64)).into(),
                Permission::ALL,
            )
            .unwrap();
        }

        // Map dead address
        mmu.set_page_table_entry(0xFFFF_FFFF_D34D_0000u64.into(), 0x0u64.into(), Permission::ALL)
            .unwrap();

        // And then write the instructions to the allocated instruction memory.
        mmu.write_bytes(0x0u64.into(), &bytes[..]).unwrap();

        // write fake return value for translation limit
        let stack_addr = stack_addr - 0x8;
        mmu.write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
            .unwrap();
    }

    // Set up the just in time compiler. TODO: Maybe we can refactor this such that the jit doesn't need the spec.
    let spec = Rc::new(RefCell::new(spec));
    let mut jit =
        X86MogiJit::new(spec.clone(), mmu.clone(), std::env::var("MOGI_DEBUG_JIT").map_or(false, |val| val == "true"));

    let mmu_tlb = mmu.clone();
    jit.register_tlb_read_miss_callback(move |ctx, addr, size, space| {
        eprintln!("tlb_read_miss: addr={addr:?}, size={size}, space={space}");

        let mmu = mmu_tlb.borrow();
        let tlb = ctx.tlb_mut();
        unsafe {
            mmu.populate_tlb(tlb, addr, MemoryAction::READ)
                .map_err(|_| todo!())
        }
    })
    .unwrap();

    let mmu_tlb = mmu.clone();
    jit.register_tlb_write_miss_callback(move |ctx, addr, size, space| {
        eprintln!("tlb_write_miss: addr={addr:?}, size={size}, space={space}");
        let mmu = mmu_tlb.borrow();
        let tlb = ctx.tlb_mut();
        unsafe {
            mmu.populate_tlb(tlb, addr, MemoryAction::WRITE)
                .map_err(|_| todo!())
        }
    })
    .unwrap();

    // Set up the cpu emulator
    let mut cpu = MogiCPU::new(jit, spec.clone(), mmu.clone());

    // Set the instruction pointer to the address we want to start executing at.
    *cpu.register_mut("RIP").unwrap() = rip;
    *cpu.register_mut("RSP").unwrap() = stack_addr - 0x8;

    (cpu, mmu, spec)
}

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("MOGI_LOG")
                .from_env_lossy(),
        )
        .init();

    let mut math_o3_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    math_o3_path.push("resources/test/executables/math_o0.o");
    let testelf_content = std::fs::read(&math_o3_path).unwrap();

    // ADD RAX, RBX
    // ADD RAX, RBX
    // ADD RAX, RBX
    // ADD RAX, RBX
    // let testelf_content =
    //     b"\x48\x01\xd8\x48\x01\xd8\x48\x01\xd8\x48\x01\xd8\x48\x01\xd8\x48\x01\xd8\x48\x01\xd8\x48\x01\xd8\xc3";

    // let (mut cpu, mmu, spec) = setup_with_bytes(&testelf_content, 0x1750u64);
    let (mut cpu, mmu, _) = setup_with_bytes(&testelf_content, 0x1620u64);
    // let (mut cpu, mmu, spec) = setup_with_bytes(testelf_content, 0x0u64);

    // write numbers to memory
    let numbers_addr = 0xFFFF_FFFF_FF00_0000u64;
    let numbers = [-9, -5, 3, -2, -7, -3, -8, -6, -2, -9, 8, 3, 8, 10, 6, -4, 5, 6, -2, 5];

    mmu.borrow_mut()
        .alloc_physical(numbers_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(numbers_addr.into(), numbers_addr.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut()
        .write(numbers_addr.into(), &numbers)
        .unwrap();

    // int *numbers
    *cpu.register_mut("RDI").unwrap() = numbers_addr;
    // size_t n
    *cpu.register_mut("RSI").unwrap() = numbers.len() as u64;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let median: u32 = *cpu.register("EAX").unwrap();
    eprintln!("median = {:#?}", median as i32);
}

fn load_specification(spec_path: &Path) -> ProcessorSpecification {
    let mut compiled_spec_path = current_dir().unwrap();
    compiled_spec_path.push(".output/x86.mcs");
    let file = File::open(&compiled_spec_path);
    let specification = match file {
        Ok(mut file) => {
            info!("Loading compiled spec from {compiled_spec_path:?}");
            let now = Instant::now();
            let spec = ProcessorSpecification::load(&mut file);
            info!("Loading took {:?}", now.elapsed());
            spec
        }
        Err(_) => {
            let spec_dir = spec_path.parent().unwrap();
            let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
            let compiler = SleighCompilerBuilder::new()
                .preprocessor_factory(factory)
                .build();
            let specification = compiler.compile(spec_path).unwrap();
            info!("Saving compiled spec to {compiled_spec_path:?}");
            let now = Instant::now();
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(compiled_spec_path)
                .unwrap();
            specification.save(&mut file);
            info!("Saving took {:?}", now.elapsed());
            specification
        }
    };
    specification
}
