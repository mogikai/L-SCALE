
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
    ("pshufhw", pshufhw),
    ("shufpd", shufpd), // Note: implemented in SLEIGH in Ghidra 10.3.
    ("pmaddwd", pmaddwd),
    ("pcmpistri", pcmpistri),
    ("palignr", palignr),
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

fn palignr(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    inputs: [crate::pcode::Value; 2],
) {
    assert_eq!(dst.size, inputs[0].size());
    assert_eq!(dst.size, 16);
    let out: u128 = cpu.read_var(dst);
    let val: u128 = cpu.read(inputs[0]);
    let shift: u32 = cpu.read(inputs[1]);
    let mask = ((1 << shift) - 1) << (128 - shift);
    let mut tmp = out.rotate_right(shift) & mask;
    tmp |= val >> shift;
    cpu.write_var(dst, tmp);
}

fn pcmpistri(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    inputs: [crate::pcode::Value; 2],
) {
    use std::arch::x86_64::*;
    // out is ECX
    let out: u32 = cpu.read_var(dst);
    let a: u128 = cpu.read(inputs[0]);
    let b: u128 = cpu.read(inputs[1]);
    let imm8: u8 = cpu.args[0] as u8;
    let result = unsafe {
        let a = _mm_loadu_si128(&a as *const u128 as *const _);
        let b = _mm_loadu_si128(&b as *const u128 as *const _);
        match imm8 {
            0 => _mm_cmpistri::<0>(a, b),
            1 => _mm_cmpistri::<1>(a, b),
            2 => _mm_cmpistri::<2>(a, b),
            3 => _mm_cmpistri::<3>(a, b),
            4 => _mm_cmpistri::<4>(a, b),
            5 => _mm_cmpistri::<5>(a, b),
            6 => _mm_cmpistri::<6>(a, b),
            7 => _mm_cmpistri::<7>(a, b),
            8 => _mm_cmpistri::<8>(a, b),
            9 => _mm_cmpistri::<9>(a, b),
            10 => _mm_cmpistri::<10>(a, b),
            11 => _mm_cmpistri::<11>(a, b),
            12 => _mm_cmpistri::<12>(a, b),
            13 => _mm_cmpistri::<13>(a, b),
            14 => _mm_cmpistri::<14>(a, b),
            15 => _mm_cmpistri::<15>(a, b),
            16 => _mm_cmpistri::<16>(a, b),
            17 => _mm_cmpistri::<17>(a, b),
            18 => _mm_cmpistri::<18>(a, b),
            19 => _mm_cmpistri::<19>(a, b),
            20 => _mm_cmpistri::<20>(a, b),
            21 => _mm_cmpistri::<21>(a, b),
            22 => _mm_cmpistri::<22>(a, b),
            23 => _mm_cmpistri::<23>(a, b),
            24 => _mm_cmpistri::<24>(a, b),
            25 => _mm_cmpistri::<25>(a, b),
            26 => _mm_cmpistri::<26>(a, b),
            27 => _mm_cmpistri::<27>(a, b),
            28 => _mm_cmpistri::<28>(a, b),
            29 => _mm_cmpistri::<29>(a, b),
            30 => _mm_cmpistri::<30>(a, b),
            31 => _mm_cmpistri::<31>(a, b),
            32 => _mm_cmpistri::<32>(a, b),
            33 => _mm_cmpistri::<33>(a, b),
            34 => _mm_cmpistri::<34>(a, b),
            35 => _mm_cmpistri::<35>(a, b),
            36 => _mm_cmpistri::<36>(a, b),
            37 => _mm_cmpistri::<37>(a, b),
            38 => _mm_cmpistri::<38>(a, b),
            39 => _mm_cmpistri::<39>(a, b),
            40 => _mm_cmpistri::<40>(a, b),
            41 => _mm_cmpistri::<41>(a, b),
            42 => _mm_cmpistri::<42>(a, b),
            43 => _mm_cmpistri::<43>(a, b),
            44 => _mm_cmpistri::<44>(a, b),
            45 => _mm_cmpistri::<45>(a, b),
            46 => _mm_cmpistri::<46>(a, b),
            47 => _mm_cmpistri::<47>(a, b),
            48 => _mm_cmpistri::<48>(a, b),
            49 => _mm_cmpistri::<49>(a, b),
            50 => _mm_cmpistri::<50>(a, b),
            51 => _mm_cmpistri::<51>(a, b),
            52 => _mm_cmpistri::<52>(a, b),
            53 => _mm_cmpistri::<53>(a, b),
            54 => _mm_cmpistri::<54>(a, b),
            55 => _mm_cmpistri::<55>(a, b),
            56 => _mm_cmpistri::<56>(a, b),
            57 => _mm_cmpistri::<57>(a, b),
            58 => _mm_cmpistri::<58>(a, b),
            59 => _mm_cmpistri::<59>(a, b),
            60 => _mm_cmpistri::<60>(a, b),
            61 => _mm_cmpistri::<61>(a, b),
            62 => _mm_cmpistri::<62>(a, b),
            63 => _mm_cmpistri::<63>(a, b),
            64 => _mm_cmpistri::<64>(a, b),
            65 => _mm_cmpistri::<65>(a, b),
            66 => _mm_cmpistri::<66>(a, b),
            67 => _mm_cmpistri::<67>(a, b),
            68 => _mm_cmpistri::<68>(a, b),
            69 => _mm_cmpistri::<69>(a, b),
            70 => _mm_cmpistri::<70>(a, b),
            71 => _mm_cmpistri::<71>(a, b),
            72 => _mm_cmpistri::<72>(a, b),
            73 => _mm_cmpistri::<73>(a, b),
            74 => _mm_cmpistri::<74>(a, b),
            75 => _mm_cmpistri::<75>(a, b),
            76 => _mm_cmpistri::<76>(a, b),
            77 => _mm_cmpistri::<77>(a, b),
            78 => _mm_cmpistri::<78>(a, b),
            79 => _mm_cmpistri::<79>(a, b),
            80 => _mm_cmpistri::<80>(a, b),
            81 => _mm_cmpistri::<81>(a, b),
            82 => _mm_cmpistri::<82>(a, b),
            83 => _mm_cmpistri::<83>(a, b),
            84 => _mm_cmpistri::<84>(a, b),
            85 => _mm_cmpistri::<85>(a, b),
            86 => _mm_cmpistri::<86>(a, b),
            87 => _mm_cmpistri::<87>(a, b),
            88 => _mm_cmpistri::<88>(a, b),
            89 => _mm_cmpistri::<89>(a, b),
            90 => _mm_cmpistri::<90>(a, b),
            91 => _mm_cmpistri::<91>(a, b),
            92 => _mm_cmpistri::<92>(a, b),
            93 => _mm_cmpistri::<93>(a, b),
            94 => _mm_cmpistri::<94>(a, b),
            95 => _mm_cmpistri::<95>(a, b),
            96 => _mm_cmpistri::<96>(a, b),
            97 => _mm_cmpistri::<97>(a, b),
            98 => _mm_cmpistri::<98>(a, b),
            99 => _mm_cmpistri::<99>(a, b),
            100 => _mm_cmpistri::<100>(a, b),
            101 => _mm_cmpistri::<101>(a, b),
            102 => _mm_cmpistri::<102>(a, b),
            103 => _mm_cmpistri::<103>(a, b),
            104 => _mm_cmpistri::<104>(a, b),
            105 => _mm_cmpistri::<105>(a, b),
            106 => _mm_cmpistri::<106>(a, b),
            107 => _mm_cmpistri::<107>(a, b),
            108 => _mm_cmpistri::<108>(a, b),
            109 => _mm_cmpistri::<109>(a, b),
            110 => _mm_cmpistri::<110>(a, b),
            111 => _mm_cmpistri::<111>(a, b),
            112 => _mm_cmpistri::<112>(a, b),
            113 => _mm_cmpistri::<113>(a, b),
            114 => _mm_cmpistri::<114>(a, b),
            115 => _mm_cmpistri::<115>(a, b),
            116 => _mm_cmpistri::<116>(a, b),
            117 => _mm_cmpistri::<117>(a, b),
            118 => _mm_cmpistri::<118>(a, b),
            119 => _mm_cmpistri::<119>(a, b),
            120 => _mm_cmpistri::<120>(a, b),
            121 => _mm_cmpistri::<121>(a, b),
            122 => _mm_cmpistri::<122>(a, b),
            123 => _mm_cmpistri::<123>(a, b),
            124 => _mm_cmpistri::<124>(a, b),
            125 => _mm_cmpistri::<125>(a, b),
            126 => _mm_cmpistri::<126>(a, b),
            127 => _mm_cmpistri::<127>(a, b),
            128 => _mm_cmpistri::<128>(a, b),
            129 => _mm_cmpistri::<129>(a, b),
            130 => _mm_cmpistri::<130>(a, b),
            131 => _mm_cmpistri::<131>(a, b),
            132 => _mm_cmpistri::<132>(a, b),
            133 => _mm_cmpistri::<133>(a, b),
            134 => _mm_cmpistri::<134>(a, b),
            135 => _mm_cmpistri::<135>(a, b),
            136 => _mm_cmpistri::<136>(a, b),
            137 => _mm_cmpistri::<137>(a, b),
            138 => _mm_cmpistri::<138>(a, b),
            139 => _mm_cmpistri::<139>(a, b),
            140 => _mm_cmpistri::<140>(a, b),
            141 => _mm_cmpistri::<141>(a, b),
            142 => _mm_cmpistri::<142>(a, b),
            143 => _mm_cmpistri::<143>(a, b),
            144 => _mm_cmpistri::<144>(a, b),
            145 => _mm_cmpistri::<145>(a, b),
            146 => _mm_cmpistri::<146>(a, b),
            147 => _mm_cmpistri::<147>(a, b),
            148 => _mm_cmpistri::<148>(a, b),
            149 => _mm_cmpistri::<149>(a, b),
            150 => _mm_cmpistri::<150>(a, b),
            151 => _mm_cmpistri::<151>(a, b),
            152 => _mm_cmpistri::<152>(a, b),
            153 => _mm_cmpistri::<153>(a, b),
            154 => _mm_cmpistri::<154>(a, b),
            155 => _mm_cmpistri::<155>(a, b),
            156 => _mm_cmpistri::<156>(a, b),
            157 => _mm_cmpistri::<157>(a, b),
            158 => _mm_cmpistri::<158>(a, b),
            159 => _mm_cmpistri::<159>(a, b),
            160 => _mm_cmpistri::<160>(a, b),
            161 => _mm_cmpistri::<161>(a, b),
            162 => _mm_cmpistri::<162>(a, b),
            163 => _mm_cmpistri::<163>(a, b),
            164 => _mm_cmpistri::<164>(a, b),
            165 => _mm_cmpistri::<165>(a, b),
            166 => _mm_cmpistri::<166>(a, b),
            167 => _mm_cmpistri::<167>(a, b),
            168 => _mm_cmpistri::<168>(a, b),
            169 => _mm_cmpistri::<169>(a, b),
            170 => _mm_cmpistri::<170>(a, b),
            171 => _mm_cmpistri::<171>(a, b),
            172 => _mm_cmpistri::<172>(a, b),
            173 => _mm_cmpistri::<173>(a, b),
            174 => _mm_cmpistri::<174>(a, b),
            175 => _mm_cmpistri::<175>(a, b),
            176 => _mm_cmpistri::<176>(a, b),
            177 => _mm_cmpistri::<177>(a, b),
            178 => _mm_cmpistri::<178>(a, b),
            179 => _mm_cmpistri::<179>(a, b),
            180 => _mm_cmpistri::<180>(a, b),
            181 => _mm_cmpistri::<181>(a, b),
            182 => _mm_cmpistri::<182>(a, b),
            183 => _mm_cmpistri::<183>(a, b),
            184 => _mm_cmpistri::<184>(a, b),
            185 => _mm_cmpistri::<185>(a, b),
            186 => _mm_cmpistri::<186>(a, b),
            187 => _mm_cmpistri::<187>(a, b),
            188 => _mm_cmpistri::<188>(a, b),
            189 => _mm_cmpistri::<189>(a, b),
            190 => _mm_cmpistri::<190>(a, b),
            191 => _mm_cmpistri::<191>(a, b),
            192 => _mm_cmpistri::<192>(a, b),
            193 => _mm_cmpistri::<193>(a, b),
            194 => _mm_cmpistri::<194>(a, b),
            195 => _mm_cmpistri::<195>(a, b),
            196 => _mm_cmpistri::<196>(a, b),
            197 => _mm_cmpistri::<197>(a, b),
            198 => _mm_cmpistri::<198>(a, b),
            199 => _mm_cmpistri::<199>(a, b),
            200 => _mm_cmpistri::<200>(a, b),
            201 => _mm_cmpistri::<201>(a, b),
            202 => _mm_cmpistri::<202>(a, b),
            203 => _mm_cmpistri::<203>(a, b),
            204 => _mm_cmpistri::<204>(a, b),
            205 => _mm_cmpistri::<205>(a, b),
            206 => _mm_cmpistri::<206>(a, b),
            207 => _mm_cmpistri::<207>(a, b),
            208 => _mm_cmpistri::<208>(a, b),
            209 => _mm_cmpistri::<209>(a, b),
            210 => _mm_cmpistri::<210>(a, b),
            211 => _mm_cmpistri::<211>(a, b),
            212 => _mm_cmpistri::<212>(a, b),
            213 => _mm_cmpistri::<213>(a, b),
            214 => _mm_cmpistri::<214>(a, b),
            215 => _mm_cmpistri::<215>(a, b),
            216 => _mm_cmpistri::<216>(a, b),
            217 => _mm_cmpistri::<217>(a, b),
            218 => _mm_cmpistri::<218>(a, b),
            219 => _mm_cmpistri::<219>(a, b),
            220 => _mm_cmpistri::<220>(a, b),
            221 => _mm_cmpistri::<221>(a, b),
            222 => _mm_cmpistri::<222>(a, b),
            223 => _mm_cmpistri::<223>(a, b),
            224 => _mm_cmpistri::<224>(a, b),
            225 => _mm_cmpistri::<225>(a, b),
            226 => _mm_cmpistri::<226>(a, b),
            227 => _mm_cmpistri::<227>(a, b),
            228 => _mm_cmpistri::<228>(a, b),
            229 => _mm_cmpistri::<229>(a, b),
            230 => _mm_cmpistri::<230>(a, b),
            231 => _mm_cmpistri::<231>(a, b),
            232 => _mm_cmpistri::<232>(a, b),
            233 => _mm_cmpistri::<233>(a, b),
            234 => _mm_cmpistri::<234>(a, b),
            235 => _mm_cmpistri::<235>(a, b),
            236 => _mm_cmpistri::<236>(a, b),
            237 => _mm_cmpistri::<237>(a, b),
            238 => _mm_cmpistri::<238>(a, b),
            239 => _mm_cmpistri::<239>(a, b),
            240 => _mm_cmpistri::<240>(a, b),
            241 => _mm_cmpistri::<241>(a, b),
            242 => _mm_cmpistri::<242>(a, b),
            243 => _mm_cmpistri::<243>(a, b),
            244 => _mm_cmpistri::<244>(a, b),
            245 => _mm_cmpistri::<245>(a, b),
            246 => _mm_cmpistri::<246>(a, b),
            247 => _mm_cmpistri::<247>(a, b),
            248 => _mm_cmpistri::<248>(a, b),
            249 => _mm_cmpistri::<249>(a, b),
            250 => _mm_cmpistri::<250>(a, b),
            251 => _mm_cmpistri::<251>(a, b),
            252 => _mm_cmpistri::<252>(a, b),
            253 => _mm_cmpistri::<253>(a, b),
            254 => _mm_cmpistri::<254>(a, b),
            255 => _mm_cmpistri::<255>(a, b),
            _ => panic!(),
        }
    };
    cpu.write_var(dst, result);
    // TODO: What about flags?
}

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
    let src = cpu.read::<u128>(args[1].slice(0, 16));
    let count = cpu.args[0] as u64;

    for offset in 0..4 {
        let shift = (count >> (offset * 2) & 0b11) * 16;
        let value = (src >> shift) & 0xffff;
        cpu.write_var(dst.slice(offset * 2, 2), value as u16);
    }
    cpu.write_var(dst.slice(8, 8), (src >> 64) as u64);
}

fn pshufhw(
    cpu: &mut crate::core::CpuCore,
    dst: crate::pcode::VarNode,
    args: [crate::pcode::Value; 2],
) {
    let src = cpu.read::<u128>(args[1].slice(0, 16));
    let count = cpu.args[0] as u64;

    let mut out = 0u128;
    // Mask out anything above the lower 64 bits
    out |= src & 0xffffffffffffffff;
    for offset in 0..4 {
        let shift = (count >> (offset * 2) & 0b11) * 16;
        let value = (src >> shift) & 0xffff0000000000000000;
        // Only set bits 64-80
        out |= value << (offset * 16);
    }
    cpu.write_var(dst, out);
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
