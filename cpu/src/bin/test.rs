extern crate core;

use std::cell::RefCell;
use std::rc::Rc;

use target_lexicon::Triple;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    fmt,
    EnvFilter,
};

use mogi_cpu::cpu::builder::CpuBuilder;
use mogi_mem::perm::{
    EXEC,
    READ,
    WRITE,
};
use mogi_mem::{
    AllocLayout,
    Mmu,
    UnallocatedMemory,
};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("FUZE_LOG"))
        .init();
    let mmu = Rc::new(RefCell::new(Mmu::new()));
    let mut cpu = CpuBuilder::new(mmu.clone())
        .target_triple(Triple::host())
        .build()
        .unwrap();

    mmu.borrow_mut()
        .alloc_memory(
            AllocLayout {
                addr: Some(0x100_000),
                size: 0x1000,
                align: 4096,
            },
            UnallocatedMemory {
                perm: READ | WRITE | EXEC,
                value: 0x90,
            },
        )
        .unwrap();
    cpu.write_reg("RAX", 42).unwrap();
    cpu.write_reg("RBX", 42).unwrap();
    cpu.write_pc(0x100_000).unwrap();
    mmu.borrow_mut()
        .write_bytes(0x100_000, &[0x48, 0x01, 0xD8], READ | WRITE | EXEC)
        .unwrap();
    mmu.borrow_mut()
        .write(0x100_010, [0x0f, 0x05], READ | WRITE | EXEC)
        .unwrap();

    let x = cpu.run();
    let val = cpu.read_reg("RAX").unwrap();
    println!("{x:?}");
    println!("RAX: {val:?}");
    let val = cpu.read_reg("RBX").unwrap();
    println!("RBX: {val:?}");
    // cpu.mem.map_physical()
}
