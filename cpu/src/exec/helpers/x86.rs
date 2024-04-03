use bitflags::bitflags;

use super::*;

pub const HELPERS: &[(&str, crate::exec::helpers::PcodeOpHelper)] = &[
    ("rdtsc", rdtsc),
    ("cpuid_basic_info", cpuid),
    ("cpuid_Version_info", cpuid),
    ("cpuid_Extended_Feature_Enumeration_info", cpuid),
    ("cpuid_Processor_Extended_States_info", cpuid),
    ("cpuid_cache_tlb_info", cpuid),
    ("cpuid", cpuid),
    ("movmskpd", movmskpd),
    ("pinsrw", pinsrw), // Note: implemented in SLEIGH in Ghidra 10.3.
    ("pshuflw", pshuflw),
    ("shufpd", shufpd), // Note: implemented in SLEIGH in Ghidra 10.3.
    ("pmaddwd", pmaddwd),
    ("in", in_io),
    ("out", out_io),
    ("LOCK", lock),
    ("UNLOCK", unlock),
    // Legacy float operations
    ("fsin", fsin),
    ("fcos", fcos),
    ("fptan", fptan),
    ("f2xm1", f2xm1),
    ("fscale", fscale),
];

fn rdtsc(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    _: [crate::pcode::Value; 2],
) {
    cpu.write_var(dst, 0_u64);
}

// Basic processor information
fn cpuid_basic_info(
    cpu: &mut crate::core::CpuCore,
    eax: crate::pcode::VarNode,
    ebx: crate::pcode::VarNode,
    edx: crate::pcode::VarNode,
    ecx: crate::pcode::VarNode,
) {
    tracing::debug!("cpuid(BASIC_INFO)");
    cpu.write_var(eax, cpu.cpu_id.level);
    cpu.write_var(ebx, u32::from_le_bytes(<[u8; 4]>::try_from(&cpu.cpu_id.vendor[0..4]).unwrap()));
    cpu.write_var(edx, u32::from_le_bytes(<[u8; 4]>::try_from(&cpu.cpu_id.vendor[4..8]).unwrap()));
    cpu.write_var(ecx, u32::from_le_bytes(<[u8; 4]>::try_from(&cpu.cpu_id.vendor[8..12]).unwrap()));
}

// Processor info and feature bits
fn cpuid_feature_info(
    cpu: &mut crate::core::CpuCore,
    eax: crate::pcode::VarNode,
    ebx: crate::pcode::VarNode,
    edx: crate::pcode::VarNode,
    ecx: crate::pcode::VarNode,
) {
    tracing::debug!("cpuid(FEATURE_INFO)");
    // Copied from `Coffee Lake` microarchitecture
    let (family, extended_family) = if cpu.cpu_id.family > 0xf {
        (0xf, cpu.cpu_id.family - 0xf)
    } else {
        (cpu.cpu_id.family & 0xf, 0u32)
    };
    let extended_model = cpu.cpu_id.model >> 4 & 0xff;
    let model = cpu.cpu_id.model & 0xf;
    let stepping = cpu.cpu_id.stepping & 0xf;
    cpu.write_var(eax, extended_family << 20 | extended_model << 16 | family << 8 | model << 4 | stepping);

    // Hack for linux
    let clflush = 8 << 8;
    // TODO: Implement apic_id
    let apic_id = 0;

    cpu.write_var(ebx, apic_id << 24 | clflush);
    if let Some(feat) = cpu.cpu_id.features.eax1 {
        cpu.write_var(edx, u32::try_from(feat.edx.bits()).unwrap());
        cpu.write_var(ecx, u32::try_from(feat.ecx.bits()).unwrap());
    }
}

// Return structured extended feature enumeration info leaf
fn cpuid_extended_feature_enumeration_info(
    cpu: &mut crate::core::CpuCore,
    count: u32,
    eax: crate::pcode::VarNode,
    ebx: crate::pcode::VarNode,
    edx: crate::pcode::VarNode,
    ecx: crate::pcode::VarNode,
) {
    tracing::debug!("cpuid(EXTENDED_FEATURE_ENUMERATION_INFO, {:#0x})", count);

    if let Some(feature) = cpu.cpu_id.features.eax7 {
        match count {
            // Returns extended feature flags in EBX, ECX, and EDX
            0x0 => {
                // TODO: This is the max value for sub leaves of EAX7.
                // Currently hard coded to 2
                cpu.write_var(eax, 2);
                cpu.write_var(ebx, feature.ecx0.ebx.bits());
                cpu.write_var(edx, feature.ecx0.edx.bits());
                cpu.write_var(ecx, feature.ecx0.ecx.bits());
            }

            // Returns extended feature flags in EAX
            0x1 => {
                // We don't support AVX-512 BFLOAT16 operations
                cpu.write_var(eax, feature.ecx1.eax.bits());
                // TODO: Support ebx for this leaf
                cpu.write_var(ebx, 0u32);
                cpu.write_var(edx, feature.ecx1.edx.bits());
                // TODO: Support ecx for this leaf
                cpu.write_var(ecx, 0u32);
            }
            0x2 => {
                cpu.write_var(eax, 0_u32);
                cpu.write_var(ebx, 0_u32);
                cpu.write_var(edx, feature.ecx2.edx.bits());
                cpu.write_var(ecx, 0_u32);
            }
            _ => {
                cpu.write_var(eax, 0_u32);
                cpu.write_var(ebx, 0_u32);
                cpu.write_var(edx, 0_u32);
                cpu.write_var(ecx, 0_u32);
            }
        }
    }
}

fn cpuid(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    if dst.size != 16 {
        tracing::warn!("Using unpatched SLEIGH specification, CPUID instruction will behave incorrectly");
        return;
    }
    let eax = dst.slice(0, 4);
    let ebx = dst.slice(4, 4);
    let edx = dst.slice(8, 4);
    let ecx = dst.slice(12, 4);
    let index: u32 = cpu.read(args[0]);
    let count: u32 = cpu.read(args[1]);
    tracing::debug!("cpuid({:#0x}, {:#0x})", index, count);
    match index {
        0x0 => cpuid_basic_info(cpu, eax, ebx, edx, ecx),
        0x1 => cpuid_feature_info(cpu, eax, ebx, edx, ecx),
        0x2 => cpuid_cache_tlb_info(cpu, eax, ebx, edx, ecx),
        0x7 => cpuid_extended_feature_enumeration_info(cpu, count, eax, ebx, edx, ecx),
        0xd => cpuid_xsave_features(cpu, count, eax, ebx, edx, ecx),
        // Hypervisor
        0x4000_0000 => {
            cpu.write_var(eax, 0_u32);
            cpu.write_var(ebx, 0_u32);
            cpu.write_var(edx, 0_u32);
            cpu.write_var(ecx, 0_u32);
        }

        // Get Highest Extended Function Implemented
        0x8000_0000 => {
            cpu.write_var(eax, 0x8000_0004_u32);
            cpu.write_var(ebx, 0_u32);
            cpu.write_var(edx, 0_u32);
            cpu.write_var(ecx, 0_u32);
        }

        0x8000_0001 => {
            let extended_family = 0x0;
            let family = 0x6;
            let extended_model = 0x9;
            let model = 0xe;

            let version: u32 = (extended_family << 20) | (extended_model << 16) | (family << 8) | (model << 4);
            cpu.write_var(eax, version);
            cpu.write_var(ebx, 0);
            cpu.write_var(edx, (1u32 << 29) | (1u32 << 11) | (1u32 << 20) | (1u32 << 26) | (1u32 << 27));
            cpu.write_var(ecx, (1u32 << 0) | (1u32 << 8));
        }
        unknown => {
            tracing::warn!("Unknown CPUID index: {:0x}", unknown);
            cpu.exception.code = crate::cpu::exception::ExceptionCode::UnknownCpuID as u32;
            cpu.exception.value = unknown as u64;
        }
    }
}

fn cpuid_xsave_features(
    cpu: &mut crate::core::CpuCore,
    count: u32,
    eax: crate::pcode::VarNode,
    ebx: crate::pcode::VarNode,
    edx: crate::pcode::VarNode,
    ecx: crate::pcode::VarNode,
) {
    cpu.write_var(eax, 0);
    cpu.write_var(ebx, 0);
    cpu.write_var(edx, 0);
    cpu.write_var(ecx, 0);
    if count == 1 {
        if let Some(feature) = cpu.cpu_id.features.eax0d {
            cpu.write_var(eax, feature.ecx1.eax.bits());
        }
    }
}

fn cpuid_cache_tlb_info(
    cpu: &mut crate::core::CpuCore,
    eax: crate::pcode::VarNode,
    ebx: crate::pcode::VarNode,
    edx: crate::pcode::VarNode,
    ecx: crate::pcode::VarNode,
) {
    // TODO
    cpu.write_var(eax, 0);
    cpu.write_var(ebx, 0);
    cpu.write_var(edx, 0);
    cpu.write_var(ecx, 0);
}

// Extract Packed Double-Precision Floating-Point Sign Mask
fn movmskpd(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    let src = cpu.read::<u128>(args[1]);
    let result = ((src >> 63) & 0b01) as u32 | ((src >> 126) & 0b10) as u32;

    // workaround SLEIGH bug? should zero extend to 64-bits
    cpu.write_var(crate::pcode::VarNode::new(dst.id, 8), result as u64);
}

// Insert word
#[allow(unused)]
fn pinsrw(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    // The byte offset to insert the word at
    let offset = 2 * (cpu.args[0] as u64).min(7);
    let src: u64 = cpu.read_dynamic(args[1]).zxt();

    cpu.write_var(dst.slice(offset as u8, 2), src as u16);
}

// Shuffle packed low words
fn pshuflw(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    let src = cpu.read::<u64>(args[1].slice(0, 8));
    let count = cpu.args[0] as u64;

    for offset in 0..4 {
        let shift = (count >> (offset * 2) & 0b11) * 16;
        let value = (src >> shift) & 0xffff;
        cpu.write_var(dst.slice(offset * 2, 2), value as u16);
    }
}

// Packed interleave shuffle
#[allow(unused)]
fn shufpd(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    let index = cpu.args[0] as u64;

    let a = if index & 0b01 == 0 { 0 } else { 8 };
    let b = if index & 0b10 == 0 { 0 } else { 8 };

    let lo = cpu.read::<u64>(args[0].slice(a, 8));
    let hi = cpu.read::<u64>(args[1].slice(b, 8));

    cpu.write_var(dst.slice(0, 8), lo);
    cpu.write_var(dst.slice(8, 8), hi);
}

// Multiply and Add Packed Integers
fn pmaddwd(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    for i in (0..dst.size).step_by(std::mem::size_of::<u32>()) {
        let lo = cpu.read::<i16>(args[0].slice(i, 2)) as i32 * cpu.read::<i16>(args[1].slice(i, 2)) as i32;

        let hi = cpu.read::<i16>(args[0].slice(i + 2, 2)) as i32 * cpu.read::<i16>(args[1].slice(i + 2, 2)) as i32;

        cpu.write_var(dst.slice(i, 4), lo.wrapping_add(hi));
    }
}

fn in_io(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    _: [crate::pcode::Value; 2],
) {
    cpu.write_trunc(dst, 0_u32);
}
fn out_io(
    _: &mut crate::core::CpuCore,
    _: crate::pcode::VarNode,
    _: [crate::pcode::Value; 2],
) {
}
fn lock(
    _: &mut crate::core::CpuCore,
    _: crate::pcode::VarNode,
    _: [crate::pcode::Value; 2],
) {
}
fn unlock(
    _: &mut crate::core::CpuCore,
    _: crate::pcode::VarNode,
    _: [crate::pcode::Value; 2],
) {
}

/// Compute the approximate of the sine of the source operand and store it in the destination
fn fsin(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    // Input is an 80-bit floating point number, but we treat it as a f64.
    let x = f64::from_bits(cpu.read::<u64>(args[0].slice(0, 8)));
    let result = x.sin();
    cpu.write_var(dst.truncate(8), result.to_bits());
}

/// Compute the approximate of the cosine of the source operand and store it in the destination
fn fcos(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    // Input is an 80-bit floating point number, but we treat it as a f64.
    let x = f64::from_bits(cpu.read::<u64>(args[0].truncate(8)));
    let result = x.cos();
    cpu.write_var(dst.truncate(8), result.to_bits());
}

/// Compute the approximate of the tangent of the source operand and store it in the destination
fn fptan(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    // Input is an 80-bit floating point number, but we treat it as a f64.
    let x = f64::from_bits(cpu.read::<u64>(args[0].truncate(8)));
    let result = x.tan();
    cpu.write_var(dst.truncate(8), result.to_bits());
}

/// Compute ST0 = 2^(ST0) - 1
fn f2xm1(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    // Input is an 80-bit floating point number, but we treat it as a f64.
    let st0 = f64::from_bits(cpu.read::<u64>(args[0].truncate(8)));
    let result = st0.exp2() - 1.0;
    cpu.write_var(dst.truncate(8), result.to_bits());
}

/// Compute ST0 = ST0 * 2^(trunc(ST1))
fn fscale(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    // Input is an 80-bit floating point number, but we treat it as a f64.
    let st0 = f64::from_bits(cpu.read::<u64>(args[0].truncate(8)));
    let st1 = f64::from_bits(cpu.read::<u64>(args[1].truncate(8)));
    let result = st0 * st1.trunc().exp2();
    cpu.write_var(dst.truncate(8), result.to_bits());
}
