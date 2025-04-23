use mogi_cpu::cpu::StopReason;
use mogi_cpu::jit::api::RuntimeError;
use mogi_cpu::new_cpu_trait::{
    CPU,
    RunLimit,
};
use mogi_system::components::mmu::MMU;
use mogi_utilities::addresses::EVAddr;
use std::num::NonZeroUsize;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};
use zerocopy::IntoBytes;

mod common;

#[test]
fn test_mov_reg_imm() {
    // MOV RAX, 0x1234
    let instr = b"\x48\xc7\xc0\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x1234u64);
}

#[test]
fn test_mov_reg_reg() {
    // MOV RAX, RBX
    let instr = b"\x48\x89\xd8";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RBX = 0x1234
    *cpu.register_mut("RBX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876u64);
}

#[test]
fn test_mov_mem_imm() {
    // MOV [RSP + 4*RSI + 8], 0x1234
    let instr = b"\x48\xC7\x44\xB4\x08\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RSI = 12
    let rsp: u64 = *cpu.register("RSP").unwrap();
    *cpu.register_mut("RSI").unwrap() = 12u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    let mut result = 0u64;
    mmu.borrow_mut()
        .read(EVAddr::from(rsp + 12 * 4 + 8), result.as_mut_bytes())
        .unwrap();
    assert_eq!(result, 0x1234);
}

#[test]
fn test_mov_mem_reg() {
    // MOV [RBP + 4*RSI + 8], RAX
    let instr = b"\x48\xC7\x44\xB5\x08\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RAX = 0x1234
    // RBP = 8
    // RSI = 12
    // => [8 + 4*12 + 8] = [64]
    *cpu.register_mut("RAX").unwrap() = 0x1234u64;
    *cpu.register_mut("RBP").unwrap() = 8u64;
    *cpu.register_mut("RSI").unwrap() = 12u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    // TODO assert memory content [64] = 0x1234
    assert_eq!(rax, 0x1234);
}

#[test]
fn test_mov_reg_mem() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::TRACE.into())
                .with_env_var("MOGI_LOG")
                .from_env_lossy(),
        )
        .init();
    // MOV EAX, [RBP - 0x4]
    let instr = b"\x8B\x45\xFC";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let stack_addr = 0xffff_ffff_ffff_1000u64;

    *cpu.register_mut("RBP").unwrap() = stack_addr;
    mmu.borrow_mut().write(EVAddr::from(stack_addr - 0x4), 0x12345678.as_bytes()).unwrap();
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x12345678);
}

#[test]
fn test_add_reg_imm() {
    // ADD RAX, 0x1234
    let instr = b"\x48\x05\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 + 0x1234);
}

#[test]
fn test_sub_reg_imm() {
    // SUB RAX, 0x1234
    let instr = b"\x48\x2d\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 - 0x1234);
}

#[test]
fn test_imul_reg_imm() {
    // IMUL RAX, 0x1234
    let instr = b"\x48\x69\xc0\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = i64::MIN;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: i64 = *cpu.register("RAX").unwrap();
    let expected = i64::MIN.wrapping_mul(0x1234);
    assert_eq!(rax, expected);
}

#[test]
fn test_idiv_reg_imm() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::TRACE.into())
                .with_env_var("MOGI_LOG")
                .from_env_lossy(),
        )
        .init();

    // IDIV RCX
    let instr = b"\x48\xF7\xF9";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let dividend = 0x9876i64;
    let divisor = 0x1234i64;
    let expected_remainder = dividend % divisor;

    *cpu.register_mut("RDX").unwrap() = 0x0i64;
    *cpu.register_mut("RAX").unwrap() = dividend;
    *cpu.register_mut("RCX").unwrap() = divisor;

    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: i64 = *cpu.register("RAX").unwrap();
    let rdx: i64 = *cpu.register("RDX").unwrap();
    let expected = dividend.wrapping_div(divisor);
    assert_eq!(rdx, expected_remainder);
    assert_eq!(rax, expected);
}

#[test]
fn test_idiv_reg_imm_divide_by_zero() {
    // IDIV RCX
    let instr = b"\x48\xF7\xF9";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let dividend = 0x9876i64;
    let divisor = 0x0i64;

    *cpu.register_mut("RDX").unwrap() = 0x0i64;
    *cpu.register_mut("RAX").unwrap() = dividend;
    *cpu.register_mut("RCX").unwrap() = divisor;

    assert!(matches!(
        cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap())),
        Err(StopReason::RuntimeError(RuntimeError::DivisionError))
    ));
}

#[test]
fn test_cqo() {
    // CQO
    let instr = b"\x48\x99";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    *cpu.register_mut("RAX").unwrap() = -1i64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: i64 = *cpu.register("RAX").unwrap();
    let rdx: i64 = *cpu.register("RDX").unwrap();
    assert_eq!(rax, -1i64);
    assert_eq!(rdx, -1i64);
}

#[test]
fn test_cdq() {
    // CDQ
    let instr = b"\x99";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    *cpu.register_mut("EAX").unwrap() = -1i32;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let eax: i32 = *cpu.register("EAX").unwrap();
    let edx: i32 = *cpu.register("EDX").unwrap();
    assert_eq!(eax, -1i32);
    assert_eq!(edx, -1i32);
}

#[test]
fn test_cwd() {
    // CWD
    let instr = b"\x66\x99";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    *cpu.register_mut("AX").unwrap() = -1i16;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let ax: i16 = *cpu.register("AX").unwrap();
    let dx: i16 = *cpu.register("DX").unwrap();
    assert_eq!(ax, -1i16);
    assert_eq!(dx, -1i16);
}

#[test]
fn test_xor_reg_imm() {
    // XOR RAX, 0x1234
    let instr = b"\x48\x35\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 ^ 0x1234);
}

#[test]
fn test_and_reg_imm() {
    // AND RAX, 0x1234
    let instr = b"\x48\x25\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 & 0x1234);
}

#[test]
fn test_or_reg_imm() {
    // OR RAX, 0x1234
    let instr = b"\x48\x0d\x34\x12\x00\x00";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    // RAX = 0x9876
    *cpu.register_mut("RAX").unwrap() = 0x9876u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x9876 | 0x1234);
}

#[test]
fn test_bsr_reg_imm() {
    // BSR RAX, RBX
    let instr = b"\x48\x0F\xBD\xC3";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    *cpu.register_mut("RBX").unwrap() = 0b1000011100u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 63 - 0b1000011100u64.leading_zeros() as u64);
}

#[test]
fn test_idiv_32() {
    // IDIV EAX
    let instr = b"\xF7\xF8";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    *cpu.register_mut("RAX").unwrap() = 0x1234i64;
    *cpu.register_mut("RDX").unwrap() = 0x0i64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: i64 = *cpu.register("RAX").unwrap();
    let rdx: i64 = *cpu.register("RDX").unwrap();
    assert_eq!(rax, 0x1234i64 / 0x1234i64);
    assert_eq!(rdx, 0x0i64);
}

#[test]
#[ignore]
fn test_simple_arithmetic() {
    let instr =
        b"\x48\x31\xC0\x48\x83\xEF\x01\x48\x01\xF8\x48\x85\xFF\x75\xF4\x48\x89\xC7\x48\xC7\xC0\x3C\x00\x00\x00\x0F\x05\xF4";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    *cpu.register_mut("RDI").unwrap() = 10u64;
    *cpu.register_mut("RSI").unwrap() = 1u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(500).unwrap()))
        .unwrap();

    let rdi: u64 = *cpu.register("RDI").unwrap();
    let rdi_expected = stuff(10, 1);

    assert_eq!(rdi, rdi_expected);
}

extern "C" fn stuff(
    n: u64,
    s: u64,
) -> u64 {
    // rdi, rsi, rdx, rcx, r8, and r9
    let mut sum = 0u64;
    for i in (0..n).step_by(s as usize) {
        sum = sum.wrapping_add(i);
    }
    sum
}
