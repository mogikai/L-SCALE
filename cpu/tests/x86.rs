use mogi_cpu::new_cpu_trait::{
    CPU,
    RunLimit,
};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
#[ignore]
fn test_mov_mem_imm() {
    // FIXME: Something is wrong with this test or the implementation of this mov.
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
    // FIXME: This test crashes when built in release mode. The compiler seems to optimize something away
    //        which it is not supposed to??
    // MOV [RBP + 4*RSI + 8], RAX
    let instr = b"\x48\xC7\x44\xB5\x08\x34\x12\x00\x00";
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
#[ignore]
fn test_imul_reg_imm() {
    // FIXME: Support 16 byte multiplication
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
fn test_cqo() {
    // CQO
    let instr = b"\x48\x99";
    let mut cpu = common::setup_with_bytes(instr);

    *cpu.register_mut("RAX").unwrap() = -1i64;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let rax: i64 = *cpu.register("RAX").unwrap();
    let rdx: i64 = *cpu.register("RDX").unwrap();
    assert_eq!(rax, -1i64);
    assert_eq!(rdx, -1i64);
}

#[test]
fn test_cdq() {
    // CDQ
    let instr = b"\x99";
    let mut cpu = common::setup_with_bytes(instr);

    *cpu.register_mut("EAX").unwrap() = -1i32;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let eax: i32 = *cpu.register("EAX").unwrap();
    let edx: i32 = *cpu.register("EDX").unwrap();
    assert_eq!(eax, -1i32);
    assert_eq!(edx, -1i32);
}

#[test]
fn test_cwd() {
    // CWD
    let instr = b"\x66\x99";
    let mut cpu = common::setup_with_bytes(instr);

    *cpu.register_mut("AX").unwrap() = -1i16;
    cpu.run(RunLimit::InstructionCount(1)).unwrap();

    let ax: i16 = *cpu.register("AX").unwrap();
    let dx: i16 = *cpu.register("DX").unwrap();
    assert_eq!(ax, -1i16);
    assert_eq!(dx, -1i16);
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
