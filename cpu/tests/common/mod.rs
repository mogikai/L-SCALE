use goblin::elf::Elf;
use mogi_cpu::cpu::MogiCPU;
use mogi_cpu::jit::api::{
    JITTaskRuntimeContext,
    MogiJit,
};
use mogi_cpu::jit::x86::X86MogiJit;
use mogi_cpu::new_cpu_trait::CPU;
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
use mogi_utilities::addresses::EVAddr;
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

#[allow(dead_code)]
pub(crate) fn dump_registers(cpu: &MogiCPU<X86MogiJit, MogiMMU>) {
    let register_names = [
        "RIP", "RAX", "RBX", "RCX", "RDX", "RDI", "RSI", "RBP", "RSP", "R8", "R9", "R10", "R11", "R12", "R13", "R14",
        "R15",
    ];

    for register_name in register_names.iter() {
        let register_value: u64 = *cpu.register(register_name).unwrap();
        eprintln!("{:#3} = {:#018X}", register_name, register_value);
    }

    let register_names = ["XMM0", "XMM1", "XMM2", "XMM3"];

    for register_name in register_names.iter() {
        let register_value: u128 = *cpu.register(register_name).unwrap();
        eprintln!("{:#3} = {:#034X}", register_name, register_value);
    }

    // also dump flags
    let flag_names = ["ZF", "SF", "CF", "OF", "PF"];
    for flag_name in flag_names.iter() {
        let flag_value: u8 = *cpu.register(flag_name).unwrap();
        eprintln!("{:#2} = {}", flag_name, flag_value);
    }

    eprintln!("");
}

#[allow(dead_code)]
pub(crate) fn dump_memory(
    cpu: &MogiCPU<X86MogiJit, MogiMMU>,
    mmu: &Rc<RefCell<MogiMMU>>,
    addr: EVAddr,
    n_bytes: usize,
) {
    for offset in (0..n_bytes).step_by(4) {
        let addr = EVAddr::try_from(addr + offset).unwrap();

        if offset % 16 == 0 {
            if offset > 0 {
                eprintln!();
            }

            eprint!("{:#018X}: ", addr);
        }

        let mut word = 0u32;
        mmu.borrow_mut().read(addr, &mut word).unwrap();

        eprint!("{:#010X} ", word);
    }

    eprintln!();
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

// Load file in `cpu/resources/tests/executables/{name}` and return the content
// as well as the address of function `fn_name`.
pub(crate) fn load_test_object(
    name: &str,
    fn_name: &str,
) -> (Vec<u8>, EVAddr) {
    let mut object_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    object_path.push(format!("resources/test/executables/{name}"));
    let content = std::fs::read(&object_path).unwrap();

    let elf = Elf::parse(&content).unwrap();
    let fn_addr = elf
        .syms
        .iter()
        .find_map(|sym| {
            if sym.is_function() && sym.st_value != 0 {
                let sym_name = elf.strtab.get_at(sym.st_name).unwrap_or("<invalid>");
                if sym_name == fn_name {
                    return Some(sym.st_value);
                }
            }

            None
        })
        .expect(&format!("Couldn't find function '{fn_name}'"));

    (content, fn_addr.into())
}
