use mogi_neocpu::cpu::CPU;
use mogi_neocpu::jit::x86::X86MogiJit;

fn main() {
    let mut cpu = CPU::new(X86MogiJit::new());
    cpu.run().unwrap();
}
