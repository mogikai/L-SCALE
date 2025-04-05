use mogi_cpu::new_cpu_trait::{
    CPU,
    RunLimit,
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

mod common;

#[test]
fn test_mov_reg_imm() {
    // MOV RAX, 0x1234
    let instr = b"\x48\xc7\xc0\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x1234u64);
}

#[test]
fn test_mov_reg_reg() {
    // MOV RAX, RBX
    let instr = b"\x48\x89\xd8";
    let mut cpu = common::setup_with_bytes(instr);

    // RBX = 0x1234
    *cpu.register_mut("RBX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876u64);
}

#[test]
fn test_mov_mem_imm() {
    // MOV [RBP + 4*RSI + 8], 0x1234
    let instr = b"\x48\xC7\x44\xB5\x08\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RBP = 8
    // RSI = 12
    // => [8 + 4*12 + 8] = [64]
    *cpu.register_mut("RBP").unwrap() = 8u64;
    *cpu.register_mut("RSI").unwrap() = 12u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    // TODO assert memory content [64] = 0x1234
    assert_eq!(rax, 0x1234);
}

#[test]
fn test_mov_mem_reg() {
    // MOV [RBP + 4*RSI + 8], RAX
    let instr = b"\x48\xc7\x44\x35\x08\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RAX = 0x1234
    // RBP = 8
    // RSI = 12
    // => [8 + 4*12 + 8] = [64]
    *cpu.register_mut("RAX").unwrap() = 0x1234u64;
    *cpu.register_mut("RBP").unwrap() = 8u64;
    *cpu.register_mut("RSI").unwrap() = 12u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    // TODO assert memory content [64] = 0x1234
    assert_eq!(rax, 0x1234);
}

#[test]
fn test_add_reg_imm() {
    // ADD RAX, 0x1234
    let instr = b"\x48\x05\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 + 0x1234);
}

#[test]
fn test_sub_reg_imm() {
    // SUB RAX, 0x1234
    let instr = b"\x48\x2d\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 - 0x1234);
}

#[test]
fn test_imul_reg_imm() {
    // IMUL RAX, 0x1234
    let instr = b"\x48\x69\xc0\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 * 0x1234);
}

#[test]
fn test_xor_reg_imm() {
    // XOR RAX, 0x1234
    let instr = b"\x48\x35\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 ^ 0x1234);
}

#[test]
fn test_and_reg_imm() {
    // AND RAX, 0x1234
    let instr = b"\x48\x25\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 & 0x1234);
}

#[test]
fn test_or_reg_imm() {
    // OR RAX, 0x1234
    let instr = b"\x48\x0d\x34\x12\x00\x00";
    let mut cpu = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 | 0x1234);
}
