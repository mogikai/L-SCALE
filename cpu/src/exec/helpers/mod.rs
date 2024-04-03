use crate::core::regs::ValueSource;
use crate::core::CpuCore;
use crate::cpu::exception::ExceptionCode;
use crate::pcode;
use crate::pcode::{
    Value,
    VarNode,
};

pub type PcodeOpHelper = fn(&mut CpuCore, VarNode, [Value; 2]);

pub fn unknown_operation(
    cpu: &mut CpuCore,
    _: VarNode,
    _: [Value; 2],
) {
    cpu.exception.code = ExceptionCode::UnimplementedOp as u32;
}

pub const HELPERS: &[(&str, PcodeOpHelper)] = &[
    ("count_leading_zeros", count_leading_zeros),
    ("count_leading_zeroes", count_leading_zeros),
    ("countLeadingZeros", count_leading_zeros),
    ("count_leading_ones", count_leading_ones),
    ("countLeadingOnes", count_leading_ones),
    ("bcd_add", bcd_add),
    ("UnsignedSaturate", unsigned_saturate),
    ("SignedSaturate", signed_saturate),
    ("SignedDoesSaturate", signed_does_saturate),
];

fn enable_interrupts(
    _cpu: &mut CpuCore,
    _: VarNode,
    _: [Value; 2],
) {
    // @todo
}

fn disable_interrupts(
    _cpu: &mut CpuCore,
    _: VarNode,
    _: [Value; 2],
) {
    // @todo
}

fn count_leading_zeros(
    cpu: &mut CpuCore,
    dst: VarNode,
    args: [Value; 2],
) {
    let input = args[0];
    let result = match args[0].size() {
        1 => cpu.read::<u8>(input).leading_zeros(),
        2 => cpu.read::<u16>(input).leading_zeros(),
        4 => cpu.read::<u32>(input).leading_zeros(),
        8 => cpu.read::<u64>(input).leading_zeros(),
        16 => cpu.read::<u128>(input).leading_zeros(),
        size => {
            cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
            cpu.exception.value = size as u64;
            return;
        }
    };
    cpu.write_trunc(dst, result);
}

fn count_leading_ones(
    cpu: &mut CpuCore,
    dst: VarNode,
    args: [Value; 2],
) {
    let input = args[0];
    let result = match args[0].size() {
        1 => cpu.read::<u8>(input).leading_ones(),
        2 => cpu.read::<u16>(input).leading_ones(),
        4 => cpu.read::<u32>(input).leading_ones(),
        8 => cpu.read::<u64>(input).leading_ones(),
        16 => cpu.read::<u128>(input).leading_ones(),
        size => {
            cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
            cpu.exception.value = size as u64;
            return;
        }
    };
    cpu.write_trunc(dst, result);
}

fn bcd_add(
    cpu: &mut CpuCore,
    dst: VarNode,
    args: [Value; 2],
) {
    let size = dst.size;

    if args[0].size() != size || args[1].size() != size {
        cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
        cpu.exception.value = args[0].size() as u64;
        return;
    }

    match size {
        1 => {
            let a = cpu.read::<u8>(args[0]);
            let b = cpu.read::<u8>(args[1]);
            let result = bcd_add8(a, b);
            cpu.write_var(dst, result);
        }
        2 => {
            let a = cpu.read::<u16>(args[0]);
            let b = cpu.read::<u16>(args[1]);
            let result = bcd_add16(a, b);
            cpu.write_var(dst, result);
        }
        _ => {
            cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
            cpu.exception.value = args[0].size() as u64;
        }
    }
}

fn bcd_add8(
    a: u8,
    b: u8,
) -> u8 {
    let mut result = 0;
    let mut carry = 0;
    for digit in 0..2 {
        let (result_digit, next_carry) = bcd_add_digit((a >> (4 * digit)) & 0xF, (b >> (4 * digit)) & 0xF, carry);
        carry = next_carry;
        result |= result_digit << (4 * digit);
    }
    result
}

fn bcd_add16(
    a: u16,
    b: u16,
) -> u16 {
    let mut result = 0;
    let mut carry = 0;
    for digit in 0..4 {
        let (result_digit, next_carry) =
            bcd_add_digit(((a >> (4 * digit)) & 0xF) as u8, ((b >> (4 * digit)) & 0xF) as u8, carry);
        carry = next_carry;
        result |= (result_digit as u16) << (4 * digit);
    }

    result
}

fn bcd_add_digit(
    a: u8,
    b: u8,
    carry: u8,
) -> (u8, u8) {
    match a + b + carry {
        x if x < 10 => (x, 0),
        x => (x % 10, 1),
    }
}

fn unsigned_saturate(
    cpu: &mut CpuCore,
    dst: pcode::VarNode,
    args: [Value; 2],
) {
    if args[1].size() != 2 {
        cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
        cpu.exception.value = args[1].size() as u64;
        return;
    }
    let bits = cpu.read::<u16>(args[1]);
    let max = (1 << bits) - 1;
    let value: u64 = cpu.read_dynamic(args[0]).zxt();
    cpu.write_trunc(dst, value.min(max));
}

fn signed_saturate(
    cpu: &mut CpuCore,
    dst: pcode::VarNode,
    args: [Value; 2],
) {
    if args[1].size() != 2 {
        cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
        cpu.exception.value = args[1].size() as u64;
        return;
    }
    let bits = cpu.read::<u16>(args[1]);
    let max = (1 << (bits - 1)) - 1;
    let min = -(1 << (bits - 1));
    let value: i64 = cpu.read_dynamic(args[0]).sxt();
    cpu.write_trunc(dst, (value).min(max).max(min) as u64);
}

fn signed_does_saturate(
    cpu: &mut CpuCore,
    dst: pcode::VarNode,
    args: [Value; 2],
) {
    if args[1].size() != 2 {
        cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
        cpu.exception.value = args[1].size() as u64;
        return;
    }
    let bits = cpu.read::<u16>(args[1]);
    let max = (1 << (bits - 1)) - 1;
    let min = -(1 << (bits - 1));
    let value: i64 = cpu.read_dynamic(args[0]).sxt();
    // Note: This seems to be impossible to implement correctly because the `SignedSaturate` is
    // applied before this step.
    cpu.write_var::<u8>(dst, (value < min || value > max) as u8);
}

#[allow(unused)]
fn saturating_sub(
    cpu: &mut CpuCore,
    dst: pcode::VarNode,
    args: [Value; 2],
) {
    let size = dst.size;

    if args[0].size() != size || args[1].size() != size {
        cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
        cpu.exception.value = args[0].size() as u64;
        return;
    }

    match size {
        1 => {
            let a = cpu.read::<u8>(args[0]);
            let b = cpu.read::<u8>(args[1]);
            cpu.write_var(dst, a.saturating_sub(b))
        }
        2 => {
            let a = cpu.read::<u16>(args[0]);
            let b = cpu.read::<u16>(args[1]);
            cpu.write_var(dst, a.saturating_sub(b))
        }
        4 => {
            let a = cpu.read::<u32>(args[0]);
            let b = cpu.read::<u32>(args[1]);
            cpu.write_var(dst, a.saturating_sub(b))
        }
        8 => {
            let a = cpu.read::<u64>(args[0]);
            let b = cpu.read::<u64>(args[1]);
            cpu.write_var(dst, a.saturating_sub(b))
        }
        _ => {
            cpu.exception.code = ExceptionCode::InvalidOpSize as u32;
            cpu.exception.value = args[0].size() as u64;
        }
    }
}

pub mod x86;

pub mod aarch64 {
    use super::*;

    pub const HELPERS: &[(&str, PcodeOpHelper)] = &[
        ("NEON_cmeq", neon_cmeq),
        ("NEON_uminv", neon_uminv),
        ("NEON_sminv", neon_sminv),
        ("NEON_umaxv", neon_umaxv),
        ("NEON_smaxv", neon_smaxv),
    ];

    //
    // NEON implementations
    // @todo: implement these in pcode
    //

    fn neon_cmeq(
        cpu: &mut CpuCore,
        dst: VarNode,
        args: [Value; 2],
    ) {
        let size = cpu.args[0] as u8;

        let a = args[0];
        let b = args[1];
        for i in (0..a.size()).step_by(size as usize) {
            let a: u64 = cpu.read_dynamic(a.slice(i, size)).zxt();
            let b: u64 = cpu.read_dynamic(b.slice(i, size)).zxt();
            cpu.write_trunc(dst.slice(i, size), if a == b { u64::MAX } else { 0 })
        }
    }

    fn neon_uminv(
        cpu: &mut CpuCore,
        dst: VarNode,
        args: [Value; 2],
    ) {
        let value = args[0];
        let size = cpu.read::<u8>(args[1]);

        let mut min = u64::MAX;
        for i in (0..value.size()).step_by(size as usize) {
            let a: u64 = cpu.read_dynamic(value.slice(i, size)).zxt();
            min = u64::min(min, a);
        }
        cpu.write_trunc(dst, min);
    }

    fn neon_sminv(
        cpu: &mut CpuCore,
        dst: VarNode,
        args: [Value; 2],
    ) {
        let value = args[0];
        let size = cpu.read::<u8>(args[1]);

        let mut min = i64::MAX;
        for i in (0..value.size()).step_by(size as usize) {
            let a: u64 = cpu.read_dynamic(value.slice(i, size)).sxt();
            min = i64::min(min, a as i64);
        }
        cpu.write_trunc(dst, min as u64);
    }

    fn neon_umaxv(
        cpu: &mut CpuCore,
        dst: VarNode,
        args: [Value; 2],
    ) {
        let value = args[0];
        let size = cpu.read::<u8>(args[1]);

        let mut max = u64::MIN;
        for i in (0..value.size()).step_by(size as usize) {
            let a = cpu.read_dynamic(value.slice(i, size)).zxt();
            max = u64::max(max, a);
        }
        cpu.write_trunc(dst, max);
    }

    fn neon_smaxv(
        cpu: &mut CpuCore,
        dst: VarNode,
        args: [Value; 2],
    ) {
        let value = args[0];
        let size = cpu.read::<u8>(args[1]);

        let mut max = i64::MIN;
        for i in (0..value.size()).step_by(size as usize) {
            let a: u64 = cpu.read_dynamic(value.slice(i, size)).sxt();
            max = i64::max(max, a as i64);
        }
        cpu.write_trunc(dst, max as u64);
    }
}

pub mod arm {
    use super::*;

    pub const HELPERS: &[(&str, PcodeOpHelper)] = &[
        ("enableIRQinterrupts", enable_interrupts),
        ("disableIRQinterrupts", disable_interrupts),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bcd_add16() {
        assert_eq!(bcd_add16(0x0001, 0x0009), 0x0010);
        assert_eq!(bcd_add16(0x0001, 0x0019), 0x0020);
        assert_eq!(bcd_add16(0x0001, 0x0099), 0x0100);
        assert_eq!(bcd_add16(0x0001, 0x0199), 0x0200);
        assert_eq!(bcd_add16(0x0001, 0x0999), 0x1000);

        assert_eq!(bcd_add16(0x1234, 0x1234), 0x2468);
        assert_eq!(bcd_add16(0x0555, 0x5555), 0x6110);
    }
}
