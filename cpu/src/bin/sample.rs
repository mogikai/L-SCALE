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
use mogi_cpu::tlb::{
    PageData,
    PageMetaInfo,
};
use mogi_mem::PAGE_SIZE;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::SleighPreprocessorFactory;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_system::components::mmu::{
    MMU,
    Permission,
};
use mogi_utilities::addresses::HVAddr;
use std::cell::RefCell;
use std::env::current_dir;
use std::fs::{
    File,
    OpenOptions,
};
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
    // let test_exe = include_bytes!("./../../../emulator/systems/ubuntu2304/home/user/main");
    let spec_path = Path::new("./deps/Ghidra/Ghidra/Processors/x86/data/languages/x86-64.slaspec");
    // let spec_path = Path::new("./cpu/Ghidra/Ghidra/Processors/x86/data/languages/x86.slaspec");
    // let spec_path = Path::new("./sleigh/ia.sinc");
    let specification = load_specification(spec_path);
    // println!("{:?}", specification.list_global_varnode_strings());

    // MOV RAX, 0x1234
    let testelf_content = b"\x48\xc7\xc0\x34\x12\x00\x00";
    let page_count = (testelf_content.len() + PAGE_SIZE - 1) / PAGE_SIZE;

    let specification = Rc::new(RefCell::new(specification));
    // Set up the mmu.
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    {
        let mut mmu = mmu.borrow_mut();
        // We allocate enough memory
        mmu.alloc_physical(0x0u64.into(), page_count * PAGE_SIZE)
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
        // And then write the instruction to the allocated memory.
        mmu.write(0x0u64.into(), &testelf_content[..]).unwrap();
    }

    let mut stack_page = Box::new(PageData::zero());
    let stack_page_addr = unsafe { HVAddr::new_unchecked(stack_page.as_mut() as *mut _) };

    // Set up the just in time compiler. TODO: Maybe we can refactor this such that the jit doesn't need the spec.
    let mut jit = X86MogiJit::new(specification.clone(), mmu.clone(), false);

    jit.register_tlb_read_miss_callback(|ctx, addr, _size, _space| {
        let tlb = ctx.tlb_mut();
        tlb.insert_read(addr, stack_page_addr);
        unsafe { Ok(stack_page_addr.byte_offset(PageData::page_offset(addr) as isize)) }
    })
    .unwrap();

    jit.register_tlb_write_miss_callback(|ctx, addr, _size, _space| {
        let tlb = ctx.tlb_mut();
        tlb.insert_write(addr, stack_page_addr);
        unsafe { Ok(stack_page_addr.byte_offset(PageData::page_offset(addr) as isize)) }
    })
    .unwrap();

    // Set up the cpu emulator
    let mut cpu = MogiCPU::new(jit, specification, mmu);

    // Set the instruction pointer to the address we want to start executing at.
    // *cpu.register_mut("RIP").unwrap() = 0x1F540u64;
    // *cpu.register_mut("RIP").unwrap() = 0x2A8E2u64;
    *cpu.register_mut("RIP").unwrap() = 0x0u64;
    *cpu.register_mut("RSP").unwrap() = 0xffff_ffff_ffff_0220u64;

    // Now lets roll! (forever :>)
    cpu.run(RunLimit::InstructionCount(1)).unwrap();
    let rax: u64 = *cpu.register("RAX").unwrap();
    eprintln!("rax = {:#?}", rax);
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
