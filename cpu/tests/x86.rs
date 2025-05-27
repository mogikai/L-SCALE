use itertools::Itertools;
use addr_align::Align;
use common::{
    dump_memory,
    dump_registers,
};
use mogi_cpu::cpu::StopReason;
use mogi_cpu::jit::api::RuntimeError;
use mogi_cpu::new_cpu_trait::{
    CPU,
    RunLimit,
};
use mogi_mem::PAGE_SIZE;
use mogi_system::components::mmu::{
    MMU,
    Permission,
};
use mogi_utilities::addresses::EVAddr;
use std::arch::breakpoint;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;
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

    let rsp: u64 = 0xFFFF_FFFF_FF00_0000u64;
    let rsi: u64 = 0x8u64;
    let addr: u64 = rsp + 4 * rsi + 8;
    let addr_aligned: u64 = addr.align_down(PAGE_SIZE as u64);
    mmu.borrow_mut()
        .alloc_physical(addr_aligned.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(addr_aligned.into(), addr_aligned.into(), Permission::ALL)
        .unwrap();

    *cpu.register_mut("RSP").unwrap() = rsp;
    *cpu.register_mut("RSI").unwrap() = rsi;
    *cpu.register_mut("RIP").unwrap() = 0x0u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let mut result = 0u64;
    mmu.borrow_mut()
        .read(EVAddr::from(addr), &mut result)
        .unwrap();

    assert_eq!(result, 0x1234);
}

#[test]
fn test_mov_mem_reg() {
    // MOV [RBP + 4*RSI + 8], RAX
    let instr = b"\x48\x89\x44\xB5\x08";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let rbp: u64 = 0xFFFF_FFFF_FFFF_0000u64;
    let rsi: u64 = 0x8u64;
    let addr: u64 = rbp + 4 * rsi + 8;

    *cpu.register_mut("RBP").unwrap() = rbp;
    *cpu.register_mut("RSI").unwrap() = rsi;
    *cpu.register_mut("RIP").unwrap() = 0x0u64;
    *cpu.register_mut("RAX").unwrap() = 0x1234u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let mut result = 0u64;
    mmu.borrow_mut()
        .read(EVAddr::from(addr), &mut result)
        .unwrap();

    assert_eq!(result, 0x1234u64, "Actual: {:#x} Expected: {:#x}", result, 0x1234u64);
}

#[test]
fn test_mov_reg_mem() {
    // MOV EAX, [RBP + 0x4]
    let instr = b"\x8B\x45\x04";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let rbp: u64 = 0xFFFF_FFFF_FFFF_0000u64;
    let addr: u64 = rbp + 4;
    mmu.borrow_mut().write(addr.into(), &0x1234).unwrap();

    *cpu.register_mut("RBP").unwrap() = rbp;
    *cpu.register_mut("RIP").unwrap() = 0x0u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x1234);
}

#[test]
fn test_movbe_mem_reg_64() {
    // MOVBE [RSP], RAX
    let instr = b"\x48\x0F\x38\xF1\x04\x24";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let rsp: u64 = *cpu.register("RSP").unwrap();
    *cpu.register_mut("RAX").unwrap() = 0x1337u64;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, 0x1337);
    let mut result = 0u64;
    mmu.borrow_mut()
        .read(EVAddr::from(rsp), &mut result)
        .unwrap();
    assert_eq!(result, 0x1337u64.swap_bytes());
}

#[test]
fn test_movbe_mem_reg_32() {
    // MOVBE [RSP], EAX
    let instr = b"\x0F\x38\xF1\x04\x24";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let rsp: u64 = *cpu.register("RSP").unwrap();
    *cpu.register_mut("EAX").unwrap() = 0x1337u32;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let rax: u32 = *cpu.register("EAX").unwrap();
    assert_eq!(rax, 0x1337);
    let mut result = 0u32;
    mmu.borrow_mut()
        .read(EVAddr::from(rsp), &mut result)
        .unwrap();
    assert_eq!(result, 0x1337u32.swap_bytes());
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
fn test_paddsw_xmm() {
    // PADDSW XMM1, XMM2
    let instr = b"\x66\x0F\xED\xCA";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let values = [-5i16, 6, -10, 45, 1003, -453, -5455, -7];
    *cpu.register_mut("XMM1").unwrap() = values;
    *cpu.register_mut("XMM2").unwrap() = values;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let xmm1: [i16; 8] = *cpu.register("XMM1").unwrap();
    assert_eq!(xmm1, values.iter().map(|x| x + x).collect::<Vec<_>>().as_slice());
}

#[test]
fn test_psubsw_xmm() {
    // PSUBSW XMM1, XMM2
    let instr = b"\x66\x0F\xE9\xCA";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let values = [-5i16, 6, -10, 45, 1003, -453, -5455, -7];
    *cpu.register_mut("XMM1").unwrap() = values;
    *cpu.register_mut("XMM2").unwrap() = values;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let xmm1: [i16; 8] = *cpu.register("XMM1").unwrap();
    assert_eq!(xmm1, values.iter().map(|x| x - x).collect::<Vec<_>>().as_slice());
}

#[test]
fn test_paddusw_xmm() {
    // PADDUSW XMM1, XMM2
    let instr = b"\x66\x0F\xDD\xCA";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let values = [5u16, 6, 10, 45, 1003, 453, 5455, 7];
    *cpu.register_mut("XMM1").unwrap() = values;
    *cpu.register_mut("XMM2").unwrap() = values;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let xmm1: [u16; 8] = *cpu.register("XMM1").unwrap();
    assert_eq!(xmm1, values.iter().map(|x| x + x).collect::<Vec<_>>().as_slice());
}

#[test]
fn test_psubusw_xmm() {
    // PSUBUSW XMM1, XMM2
    let instr = b"\x66\x0F\xD9\xCA";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let values = [5u16, 6, 10, 45, 1003, 453, 5455, 7];
    *cpu.register_mut("XMM1").unwrap() = values;
    *cpu.register_mut("XMM2").unwrap() = values;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let xmm1: [u16; 8] = *cpu.register("XMM1").unwrap();
    assert_eq!(xmm1, values.iter().map(|x| x - x).collect::<Vec<_>>().as_slice());
}

/* Test not working, because shift operations for sizes greater than 8 are not yet implemented
#[test]
fn test_pavgw_xmm() {
    // PAVGW XMM1, XMM2
    let instr = b"\x66\x0F\xE3\xCA";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let values = [5u16, 6, 10, 45, 1003, 453, 5455, 7];
    *cpu.register_mut("XMM1").unwrap() = values;
    *cpu.register_mut("XMM2").unwrap() = values;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let avg = |a: u16, b: u16| {
        let sum = a.saturating_add(b.saturating_add(1));
        sum >> 1
    };
    let xmm1: [u16; 8] = *cpu.register("XMM1").unwrap();
    assert_eq!(xmm1, values.iter().map(|x| avg(x,x)).collect::<Vec<_>>().as_slice());
}
*/

#[test]
fn test_phaddw_xmm() {
    // PHADDW XMM1, XMM2
    let instr = b"\x66\x0F\x38\x01\xCA";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let values = [-5i16, 6, -10, 45, 1003, -453, -5455, -7];
    *cpu.register_mut("XMM1").unwrap() = values;
    *cpu.register_mut("XMM2").unwrap() = values;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let values_iter = values
        .iter()
        .copied()
        .chain(values.iter().copied())
        .tuples()
        .map(|(a, b)| a.saturating_add(b));
    let chunks = values_iter.clone().chunks(values.len() / 4);
    let chunks2 = values_iter.clone().chunks(values.len() / 4);
    let first_iter = chunks.into_iter().step_by(2);
    let second_iter = chunks2.into_iter().skip(1).step_by(2);

    let xmm1: [i16; 8] = *cpu.register("XMM1").unwrap();
    assert_eq!(
        xmm1,
        first_iter
            .chain(second_iter)
            .flatten()
            .collect::<Vec<_>>()
            .as_slice()
    );
}

#[test]
fn test_paddsb_xmm() {
    // PADDSB XMM1, XMM2
    let instr = b"\x66\x0F\xEC\xCA";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let values = [-5i8, 6, -10, 45, 103, -43, -55, -7, -1, 2, -3, 0, -48, 2, -7, 82];
    *cpu.register_mut("XMM1").unwrap() = values;
    *cpu.register_mut("XMM2").unwrap() = values;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(1).unwrap()))
        .unwrap();

    let xmm1: [i8; 16] = *cpu.register("XMM1").unwrap();
    assert_eq!(
        xmm1,
        values
            .iter()
            .map(|x| x.saturating_add(*x))
            .collect::<Vec<_>>()
            .as_slice()
    );
}

#[test]
fn test_simple_arithmetic() {
    // xor rax, rax
    // loop:
    // sub rdi, 1
    // add rax, rdi
    // test rdi, rdi
    // jnz loop
    // mov rdi, rax
    // mov rax, 60
    // syscall
    // hlt
    let instr =
        b"\x48\x31\xC0\x48\x83\xEF\x01\x48\x01\xF8\x48\x85\xFF\x75\xF4\x48\x89\xC7\x48\xC7\xC0\x3C\x00\x00\x00\x0F\x05\xF4";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    *cpu.register_mut("RDI").unwrap() = 10u64;
    *cpu.register_mut("RSI").unwrap() = 1u64;
    let result = cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(500).unwrap()));
    // The code performs a syscall. We assert here that the syscall was called.
    // Syscall number should be in rax and have the value 60
    assert!(matches!(result, Err(StopReason::RuntimeError(RuntimeError::CustomOp(_)))));
    assert_eq!(*cpu.register::<u64>("RAX").unwrap(), 60u64);

    let rdi: u64 = *cpu.register("RDI").unwrap();
    let rdi_expected = stuff(10, 1);

    assert_eq!(rdi, rdi_expected);
}

#[test]
fn test_swap() {
    // MOV RAX, [RDI]
    // MOV RBX, [RSI]
    // MOV [RSI], RAX
    // MOV [RDI], RBX
    let instr = b"\x48\x8b\x07\x48\x8b\x1e\x48\x89\x06\x48\x89\x1f";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let page = 0x2000u64;
    let rdi_ptr = 0x2040u64;
    let rsi_ptr = 0x2080u64;
    let rdi_val = 0x1234u64;
    let rsi_val = 0x4321u64;
    mmu.borrow_mut()
        .alloc_physical(page.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(page.into(), page.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut().write(rdi_ptr.into(), &rdi_val).unwrap();
    mmu.borrow_mut().write(rsi_ptr.into(), &rsi_val).unwrap();

    *cpu.register_mut("RDI").unwrap() = rdi_ptr;
    *cpu.register_mut("RSI").unwrap() = rsi_ptr;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(4).unwrap()))
        .unwrap();

    let mut rdi_result: u64 = 0;
    let mut rsi_result: u64 = 0;
    mmu.borrow_mut()
        .read(rdi_ptr.into(), &mut rdi_result)
        .unwrap();
    mmu.borrow_mut()
        .read(rsi_ptr.into(), &mut rsi_result)
        .unwrap();
    assert_eq!(rdi_result, rsi_val);
    assert_eq!(rsi_result, rdi_val);
}

#[test]
fn test_swap_long() {
    // PUSH RBP
    // MOV RBP, RSP
    // MOV QWORD [VAR_8H], RDI
    // MOV QWORD [VAR_10H], RSI
    // MOV RAX, QWORD [VAR_8H]
    // MOV EAX, DWORD [RAX]
    // MOV DWORD [VAR_14H], EAX
    // MOV RAX, QWORD [VAR_10H]
    // MOV ECX, DWORD [RAX]
    // MOV RAX, QWORD [VAR_8H]
    // MOV DWORD [RAX], ECX
    // MOV ECX, DWORD [VAR_14H]
    // MOV RAX, QWORD [VAR_10H]
    // MOV DWORD [RAX], ECX
    // POP RBP

    let instr = b"\x55\x48\x89\xe5\x48\x89\x7d\xf8\x48\x89\x75\xf0\x48\x8b\x45\xf8\x8b\x00\x89\x45\xec\x48\x8b\x45\xf0\x8b\x08\x48\x8b\x45\xf8\x89\x08\x8b\x4d\xec\x48\x8b\x45\xf0\x89\x08\x5d";
    let (mut cpu, mmu) = common::setup_with_bytes(instr);

    let page = 0x2000u64;
    let rdi_ptr = 0x2042u64;
    let rsi_ptr = 0x2087u64;
    let rdi_val = 0x1234u64;
    let rsi_val = 0x4321u64;
    mmu.borrow_mut()
        .alloc_physical(page.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(page.into(), page.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut().write(rdi_ptr.into(), &rdi_val).unwrap();
    mmu.borrow_mut().write(rsi_ptr.into(), &rsi_val).unwrap();

    *cpu.register_mut("RDI").unwrap() = rdi_ptr;
    *cpu.register_mut("RSI").unwrap() = rsi_ptr;
    cpu.run(RunLimit::InstructionCount(NonZeroUsize::new(15).unwrap()))
        .unwrap();

    let mut rdi_result: u64 = 0;
    let mut rsi_result: u64 = 0;
    mmu.borrow_mut()
        .read(rdi_ptr.into(), &mut rdi_result)
        .unwrap();
    mmu.borrow_mut()
        .read(rsi_ptr.into(), &mut rsi_result)
        .unwrap();
    assert_eq!(rdi_result, rsi_val);
    assert_eq!(rsi_result, rdi_val);
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

// ++++++++ tests from resources/test/executables/misc.c ++++++++

fn is_prime(i: u64) -> bool {
    for j in 2..i {
        if i % j == 0 {
            return false;
        }
    }

    true
}

#[test]
fn test_misc_is_prime_o0() {
    let mut misc_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o0_path.push("resources/test/executables/misc_o0.o");

    let instr = &std::fs::read(&misc_o0_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let stack_addr = 0xffff_ffff_ffff_0ff0u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    for i in 2..100 {
        let rax_expected = is_prime(i) as u64;

        *cpu.register_mut("RIP").unwrap() = 0x1130u64;
        *cpu.register_mut("RSP").unwrap() = stack_addr;
        *cpu.register_mut("RDI").unwrap() = i as u64;
        cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
            .unwrap();

        let rax: u64 = *cpu.register("RAX").unwrap();
        assert_eq!(rax, rax_expected);
    }
}

#[test]
fn test_misc_is_prime_o3() {
    let mut misc_o3_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o3_path.push("resources/test/executables/misc_o3.o");

    let instr = &std::fs::read(&misc_o3_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let stack_addr = 0xffff_ffff_ffff_0ff0u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    for i in 2..100 {
        let rax_expected = is_prime(i) as u64;

        *cpu.register_mut("RIP").unwrap() = 0x1130u64;
        *cpu.register_mut("RSP").unwrap() = stack_addr;
        *cpu.register_mut("RDI").unwrap() = i as u64;
        cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
            .unwrap();

        let rax: u64 = *cpu.register("RAX").unwrap();
        assert_eq!(rax, rax_expected);
    }
}

fn nth_fibonacci(n: u64) -> u64 {
    if n < 2 {
        return 1;
    }

    let mut a = 1;
    let mut b = 1;
    for _ in 0..n {
        let sum = a + b;
        a = b;
        b = sum;
    }

    b
}

#[test]
fn test_misc_nth_fibonacci_o0() {
    let mut misc_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o0_path.push("resources/test/executables/misc_o0.o");

    let instr = &std::fs::read(&misc_o0_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let stack_addr = 0xffff_ffff_ffff_0ff0u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    for i in 0..20 {
        let rax_expected = nth_fibonacci(i);

        *cpu.register_mut("RIP").unwrap() = 0x11A0u64;
        *cpu.register_mut("RSP").unwrap() = stack_addr;
        *cpu.register_mut("RDI").unwrap() = i as u64;
        cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
            .unwrap();

        let rax: u64 = *cpu.register("RAX").unwrap();
        assert_eq!(rax, rax_expected);
    }
}

#[test]
fn test_misc_nth_fibonacci_o3() {
    let mut misc_o3_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o3_path.push("resources/test/executables/misc_o3.o");

    let instr = &std::fs::read(&misc_o3_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let stack_addr = 0xffff_ffff_ffff_0ff0u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    for i in 0..20 {
        let rax_expected = nth_fibonacci(i);

        *cpu.register_mut("RIP").unwrap() = 0x1180u64;
        *cpu.register_mut("RSP").unwrap() = stack_addr;
        *cpu.register_mut("RDI").unwrap() = i as u64;
        cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
            .unwrap();

        let rax: u64 = *cpu.register("RAX").unwrap();
        assert_eq!(rax, rax_expected);
    }
}

#[test]
fn test_misc_max_byte_o0() {
    let mut misc_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o0_path.push("resources/test/executables/misc_o0.o");

    let instr = &std::fs::read(&misc_o0_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let stack_addr = 0xffff_ffff_ffff_1000u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // write input to memory
    let bytes_addr = 0xFFFF_FFFF_FF00_0000u64;
    let bytes = [
        0x7fu8, 0x40, 0x3e, 0x48, 0x38, 0x3a, 0x7c, 0x3e, 0xa5, 0x1d, 0x30, 0x76, 0xd, 0xc8, 0xc7, 0x2c, 0xe0, 0xd6,
        0x27, 0x31,
    ];

    mmu.borrow_mut()
        .alloc_physical(bytes_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(bytes_addr.into(), bytes_addr.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut().write(bytes_addr.into(), &bytes).unwrap();

    let rax_expected = *bytes.iter().max().unwrap() as u64;

    *cpu.register_mut("RIP").unwrap() = 0x1220u64;
    *cpu.register_mut("RSP").unwrap() = stack_addr;
    *cpu.register_mut("RDI").unwrap() = bytes_addr;
    *cpu.register_mut("RSI").unwrap() = bytes.len() as u64;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, rax_expected);
}

#[test]
fn test_misc_max_byte_o3() {
    // TODO this fails because some custom pcode-ops are not yet implemented
    let mut misc_o3_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o3_path.push("resources/test/executables/misc_o3.o");

    let instr = &std::fs::read(&misc_o3_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(instr);
    let stack_addr = 0xffff_ffff_ffff_0ff0u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // write input to memory
    let bytes_addr = 0xFFFF_FFFF_FF00_0000u64;
    let bytes = [
        0x7fu8, 0x40, 0x3e, 0x48, 0x38, 0x3a, 0x7c, 0x3e, 0xa5, 0x1d, 0x30, 0x76, 0xd, 0xc8, 0xc7, 0x2c, 0xe0, 0xd6,
        0x27, 0x31,
    ];

    mmu.borrow_mut()
        .alloc_physical(bytes_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(bytes_addr.into(), bytes_addr.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut().write(bytes_addr.into(), &bytes).unwrap();

    let rax_expected = *bytes.iter().max().unwrap() as u64;

    *cpu.register_mut("RIP").unwrap() = 0x1200u64;
    *cpu.register_mut("RSP").unwrap() = stack_addr;
    *cpu.register_mut("RDI").unwrap() = bytes_addr;
    *cpu.register_mut("RSI").unwrap() = bytes.len() as u64;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let rax: u64 = *cpu.register("RAX").unwrap();
    assert_eq!(rax, rax_expected);
}

#[test]
fn test_misc_partition_o0() {
    let mut misc_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o0_path.push("resources/test/executables/misc_o0.o");

    let instr = std::fs::read(&misc_o0_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_1000u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // write input to memory
    let numbers_addr = 0xFFFF_FFFF_FF00_0000u64;
    const numbers: [i32; 32] = [
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
        .write(numbers_addr.into(), &numbers)
        .unwrap();

    let mut numbers_sorted = numbers.clone();
    numbers_sorted.sort();

    // set registers and run
    *cpu.register_mut("RIP").unwrap() = 0x1330u64;
    *cpu.register_mut("RSP").unwrap() = stack_addr;
    *cpu.register_mut("RDI").unwrap() = numbers_addr;
    *cpu.register_mut("RSI").unwrap() = 0u64;
    *cpu.register_mut("RDX").unwrap() = numbers.len() as u64 - 1;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let pi = 16usize;
    let pivot = *numbers.last().unwrap();

    // assert correct output
    let mut output = [0i32; numbers.len()];
    mmu.borrow_mut()
        .read(numbers_addr.into(), &mut output)
        .unwrap();
    let rax: u64 = *cpu.register("RAX").unwrap();

    assert_eq!(rax, pi as u64);

    for i in 0..output.len() {
        if i < pi {
            assert!(output[i] <= pivot);
        } else {
            assert!(output[i] >= pivot);
        }
    }
}

#[test]
fn test_misc_partition_o3() {
    let mut misc_o3_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o3_path.push("resources/test/executables/misc_o3.o");

    let instr = std::fs::read(&misc_o3_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_1000u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // write input to memory
    let numbers_addr = 0xFFFF_FFFF_FF00_0000u64;
    const numbers: [i32; 32] = [
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
        .write(numbers_addr.into(), &numbers)
        .unwrap();

    let mut numbers_sorted = numbers.clone();
    numbers_sorted.sort();

    // set registers and run
    *cpu.register_mut("RIP").unwrap() = 0x1390u64;
    *cpu.register_mut("RSP").unwrap() = stack_addr;
    *cpu.register_mut("RDI").unwrap() = numbers_addr;
    *cpu.register_mut("RSI").unwrap() = 0u64;
    *cpu.register_mut("RDX").unwrap() = numbers.len() as u64 - 1;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let pi = 16usize;
    let pivot = *numbers.last().unwrap();

    // assert correct output
    let mut output = [0; numbers.len()];
    mmu.borrow_mut()
        .read(numbers_addr.into(), &mut output)
        .unwrap();
    let rax: u64 = *cpu.register("RAX").unwrap();

    assert_eq!(rax, pi as u64);

    for i in 0..output.len() {
        if i < pi {
            assert!(output[i] <= pivot);
        } else {
            assert!(output[i] >= pivot);
        }
    }
}

#[test]
fn test_misc_quicksort_o0() {
    let mut misc_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o0_path.push("resources/test/executables/misc_o0.o");

    let instr = std::fs::read(&misc_o0_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_1000u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // write input to memory
    let numbers_addr = 0xFFFF_FFFF_FF00_0000u64;
    const numbers: [i32; 32] = [
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
        .write(numbers_addr.into(), &numbers)
        .unwrap();

    let mut bytes_sorted = numbers.clone();
    bytes_sorted.sort();

    *cpu.register_mut("RIP").unwrap() = 0x1400u64;
    *cpu.register_mut("RSP").unwrap() = stack_addr;
    *cpu.register_mut("RDI").unwrap() = numbers_addr;
    *cpu.register_mut("RSI").unwrap() = 0u64;
    *cpu.register_mut("RDX").unwrap() = numbers.len() as u64 - 1;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let mut output = vec![0i32; numbers.len()];
    mmu.borrow_mut()
        .read_bytes(numbers_addr.into(), output.as_mut_bytes())
        .unwrap();

    assert_eq!(output, bytes_sorted);
}

#[test]
fn test_misc_quicksort_o3() {
    let mut misc_o3_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    misc_o3_path.push("resources/test/executables/misc_o3.o");

    let instr = std::fs::read(&misc_o3_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_0FF0u64;

    // write fake return value for translation limit
    let stack_addr = stack_addr - 0x8;
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // write input to memory
    let numbers_addr = 0xFFFF_FFFF_FF00_0000u64;
    let numbers = [
        -13935i32, 17471, -23649, 20073, 18135, 11677, 28681, 7075, -31766, 27292, 32352, -13904, 8405, -1462, -10314,
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
        .write(numbers_addr.into(), &numbers)
        .unwrap();

    let mut numbers_sorted = numbers.clone();
    numbers_sorted.sort();

    *cpu.register_mut("RIP").unwrap() = 0x1450u64;
    *cpu.register_mut("RSP").unwrap() = stack_addr;
    *cpu.register_mut("RDI").unwrap() = numbers_addr;
    *cpu.register_mut("RSI").unwrap() = 0u64;
    *cpu.register_mut("RDX").unwrap() = numbers.len() as u64 - 1;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let mut output = vec![0; numbers.len()];
    mmu.borrow_mut()
        .read_bytes(numbers_addr.into(), output.as_mut_bytes())
        .unwrap();

    assert_eq!(output, numbers_sorted);
}

// -------- tests from resources/test/executables/misc.c --------

// ++++++++ tests from resources/test/executables/misc.c ++++++++
#[test]
fn test_math_prime_factors_o0() {
    let mut math_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    math_o0_path.push("resources/test/executables/math_o0.o");

    let instr = std::fs::read(&math_o0_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_0000u64;

    // write fake return value for translation limit
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // allocate memory for factors and n_factors
    let factors_addr = 0x5000_FFFF_FF00_0000u64;
    let n_factors_addr = 0x5000_FFFF_FF00_1000u64;

    mmu.borrow_mut()
        .alloc_physical(factors_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .alloc_physical(n_factors_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(factors_addr.into(), factors_addr.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(n_factors_addr.into(), n_factors_addr.into(), Permission::ALL)
        .unwrap();

    *cpu.register_mut("RIP").unwrap() = 0x1920u64;

    let factors = vec![2, 2, 3, 3, 3, 5, 5, 7, 11, 13];

    // int input
    let input: u64 = factors.iter().copied().reduce(|acc, v| acc * v).unwrap() as u64;
    *cpu.register_mut("RDI").unwrap() = input;
    // int *factors
    *cpu.register_mut("RSI").unwrap() = factors_addr;
    // size_t n_factors
    *cpu.register_mut("RDX").unwrap() = n_factors_addr;

    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let mut n_factors = 0usize;

    mmu.borrow_mut()
        .read(n_factors_addr.into(), &mut n_factors)
        .unwrap();

    let mut calculated_factors = vec![0i32; n_factors];

    mmu.borrow_mut()
        .read_bytes(factors_addr.into(), calculated_factors[..].as_mut_bytes())
        .unwrap();

    assert_eq!(calculated_factors, factors);
    assert_eq!(n_factors, factors.len());
}

#[test]
fn test_math_prime_factors_o3() {
    let mut math_o3_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    math_o3_path.push("resources/test/executables/math_o3.o");

    let instr = std::fs::read(&math_o3_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_0000u64;

    // write fake return value for translation limit
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

    // allocate memory for factors and n_factors
    let factors_addr = 0xFFFF_FFFF_FF00_0000u64;
    let n_factors_addr = 0xFFFF_FFFF_FF00_1000u64;

    mmu.borrow_mut()
        .alloc_physical(factors_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .alloc_physical(n_factors_addr.into(), 1 * PAGE_SIZE)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(factors_addr.into(), factors_addr.into(), Permission::ALL)
        .unwrap();
    mmu.borrow_mut()
        .set_page_table_entry(n_factors_addr.into(), n_factors_addr.into(), Permission::ALL)
        .unwrap();

    *cpu.register_mut("RIP").unwrap() = 0x1A50u64;

    // int input
    let input: u64 = 2 * 2 * 3 * 3 * 3 * 5 * 5 * 7 * 11 * 13;
    *cpu.register_mut("RDI").unwrap() = input;
    // int *factors
    *cpu.register_mut("RSI").unwrap() = factors_addr;
    // size_t n_factors
    *cpu.register_mut("RDX").unwrap() = n_factors_addr;
    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let mut n_factors = 0usize;

    mmu.borrow_mut()
        .read(n_factors_addr.into(), &mut n_factors)
        .unwrap();

    let mut factors = vec![0i32; n_factors];

    mmu.borrow_mut()
        .read_bytes(factors_addr.into(), factors[..n_factors].as_mut_bytes())
        .unwrap();

    assert_eq!(n_factors, 10);
    assert_eq!(factors, vec![2, 2, 3, 3, 3, 5, 5, 7, 11, 13]);
}

#[test]
fn test_math_median_o0() {
    let mut math_o0_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    math_o0_path.push("resources/test/executables/math_o0.o");

    let instr = std::fs::read(&math_o0_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_0000u64;

    // write fake return value for translation limit
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

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

    *cpu.register_mut("RIP").unwrap() = 0x1620u64;
    // int *numbers
    *cpu.register_mut("RDI").unwrap() = numbers_addr;
    // size_t n
    *cpu.register_mut("RSI").unwrap() = numbers.len() as u64;

    cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        .unwrap();

    let median: u32 = *cpu.register("EAX").unwrap();
    assert_eq!(median as i32, -2);
}

#[test]
fn test_math_median_o3() {
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
    math_o3_path.push("resources/test/executables/math_o3.o");

    let instr = std::fs::read(&math_o3_path).unwrap();
    let (mut cpu, mmu) = common::setup_with_bytes(&instr);
    let stack_addr = 0xFFFF_FFFF_FFFF_0000u64;

    // write fake return value for translation limit
    mmu.borrow_mut()
        .write(stack_addr.into(), &0xFFFF_FFFF_D34D_0000u64)
        .unwrap();

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

    *cpu.register_mut("RIP").unwrap() = 0x1750u64;
    // int *numbers
    *cpu.register_mut("RDI").unwrap() = numbers_addr;
    // size_t n
    *cpu.register_mut("RSI").unwrap() = numbers.len() as u64;
        // cpu.run(RunLimit::Address(0xFFFF_FFFF_D34D_0000u64.into()))
        //     .unwrap();
    loop {

        dump_memory(&cpu, &mmu, (stack_addr - 0x200).into(), 0x300);
        dump_registers(&cpu);
        cpu.run(RunLimit::BlockCount(NonZeroUsize::try_from(1).unwrap()))
            .unwrap();
        if *cpu.register::<u64>("RIP").unwrap() > 0xFFFF_FFFF_0000_0000 {
            break;
        }
    }

    let median: u32 = *cpu.register("EAX").unwrap();
    assert_eq!(median as i32, -2);
}
