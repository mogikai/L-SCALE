use crate::cpu::api::CpuError;
use bitflags::bitflags;
use serde::de::Error as DeError;
use serde::ser::Error as SerError;
use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
};
use std::path::{
    Path,
    PathBuf,
};

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX1 {
    pub edx: EAX1_EDX,
    pub ecx: EAX1_ECX,
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX1_EDX: u32 {
        const FP87      = 1<< 0;
        const VME       = 1<< 1;
        const DE        = 1<< 2;
        const PSE       = 1<< 3;
        const TSC       = 1<< 4;
        const MSR       = 1<< 5;
        const PAE       = 1<< 6;
        const MCE       = 1<< 7;
        const CX8       = 1<< 8;
        const APIC      = 1<< 9;
        const SEP       = 1<< 11; /* sysenter/sysexit */
        const MTRR      = 1<< 12;
        const PGE       = 1<< 13;
        const MCA       = 1<< 14;
        const CMOV      = 1<< 15;
        const PAT       = 1<< 16;
        const PSE36     = 1<< 17;
        const PN        = 1<< 18;
        const CLFLUSH   = 1<< 19;
        const DTS       = 1<< 21;
        const ACPI      = 1<< 22;
        const MMX       = 1<< 23;
        const FXSR      = 1<< 24;
        const SSE       = 1<< 25;
        const SSE2      = 1<< 26;
        const SS        = 1<< 27;
        const HT        = 1<< 28;
        const TM        = 1<< 29;
        const IA64      = 1<< 30;
        const PBE       = 1<< 31;
    }
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX1_ECX: u32 {
        const SSE3                  = 1 << 0;
        const PCLMULQDQ             = 1 << 1;
        const DTES64                = 1 << 2;
        const MONITOR               = 1 << 3;
        const DSCPL                 = 1 << 4;
        const VMX                   = 1 << 5;
        const SMX                   = 1 << 6;
        const EST                   = 1 << 7;
        const TM2                   = 1 << 8;
        const SSSE3                 = 1 << 9;
        const CID                   = 1 << 10;
        const FMA                   = 1 << 12;
        const CX16                  = 1 << 13;
        const XTPR                  = 1 << 14;
        const PDCM                  = 1 << 15;
        const PCID                  = 1 << 17;
        const DCA                   = 1 << 18;
        const SSE41                 = 1 << 19;
        const SSE42                 = 1 << 20;
        const X2APIC                = 1 << 21;
        const MOVBE                 = 1 << 22;
        const POPCNT                = 1 << 23;
        const TSC_DEADLINE_TIMER    = 1 << 24;
        const AES                   = 1 << 25;
        const XSAVE                 = 1 << 26;
        const OSXSAVE               = 1 << 27;
        const AVX                   = 1 << 28;
        const F16C                  = 1 << 29;
        const RDRAND                = 1 << 30;
        const HYPERVISOR            = 1 << 31;
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX6 {
    pub eax: EAX6_EAX,
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX6_EAX: u32 {
        const DTS                               = 1 << 0;
        const ARAT                              = 1 << 2;
        const PLN                               = 1 << 4;
        const ECMD                              = 1 << 5;
        const PTM                               = 1 << 6;
        const HWP                               = 1 << 7;
        const HWP_NOTIFICATION                  = 1 << 8;
        const HWP_ACTIVITY_WINDOW               = 1 << 9;
        const HWP_ENERGY_PERFORMANCE_PREFERENCE = 1 << 10;
        const HWP_PACKAGE_LEVEL_REQUEST         = 1 << 11;
        const HDC                               = 1 << 13;
        const INTEL_TURBO_BOOST                 = 1 << 14;
        const HW_FEEDBACK                       = 1 << 19;
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX7 {
    pub ecx0: EAX7_ECX0,
    pub ecx1: EAX7_ECX1,
    pub ecx2: EAX7_ECX2,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX7_ECX0 {
    pub ebx: EAX7_ECX0_EBX,
    pub ecx: EAX7_ECX0_ECX,
    pub edx: EAX7_ECX0_EDX,
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX7_ECX0_EBX: u32 {
        const FSGSBASE      = 1 << 0;
        const SGX           = 1 << 2;
        const BMI1          = 1 << 3;
        const HLE           = 1 << 4;
        const AVX2          = 1 << 5;
        const SMEP          = 1 << 7;
        const BMI2          = 1 << 8;
        const ERMS          = 1 << 9;
        const INVPCID       = 1 << 10;
        const RTM           = 1 << 11;
        const MPX           = 1 << 14;
        const AVX512F       = 1 << 16;
        const AVX512DQ      = 1 << 17;
        const RDSEED        = 1 << 18;
        const ADX           = 1 << 19;
        const SMAP          = 1 << 20;
        const AVX512IFMA    = 1 << 21;
        const PCOMMIT       = 1 << 22;
        const CLFLUSHOPT    = 1 << 23;
        const CLWB          = 1 << 24;
        const INTEL_PT      = 1 << 25;
        const AVX512PF      = 1 << 26;
        const AVX512ER      = 1 << 27;
        const AVX512CD      = 1 << 28;
        const SHA_NI        = 1 << 29;
        const AVX512BW      = 1 << 30;
        const AVX512VL      = 1 << 31;
    }
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX7_ECX0_ECX: u32 {
        const AVX512_VBMI       = 1 << 1;
        const UMIP              = 1 << 2;
        const PKU               = 1 << 3;
        const OSPKE             = 1 << 4;
        const WAITPKG           = 1 << 5;
        const AVX512_VBMI2      = 1 << 6;
        const GFNI              = 1 << 8;
        const VAES              = 1 << 9;
        const VPCLMULQDQ        = 1 << 10;
        const AVX512VNNI        = 1 << 11;
        const AVX512BITALG      = 1 << 12;
        const AVX512_VPOPCNTDQ  = 1 << 14;
        const LA57              = 1 << 16;
        const RDPID             = 1 << 22;
        const BUS_LOCK_DETECT   = 1 << 24;
        const CLDEMOTE          = 1 << 25;
        const MOVDIRI           = 1 << 27;
        const MOVDIR64B         = 1 << 28;
        const SGX_LC            = 1 << 30;
        const PKS               = 1 << 31;
    }
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX7_ECX0_EDX: u32 {
        const AVX512_4VNNIW         = 1 << 2;
        const AVX512_4FMAPS         = 1 << 3;
        const FSRM                  = 1 << 4;
        const AVX512_VP2INTERSECT   = 1 << 8;
        const SERIALIZE             = 1 << 14;
        const TSX_LDTRK             = 1 << 16;
        const ARCH_LBR              = 1 << 19;
        const AMX_BF16              = 1 << 22;
        const AVX512_FP16           = 1 << 23;
        const AMX_TILE              = 1 << 24;
        const AMX_INT8              = 1 << 25;
        const SPEC_CTRL             = 1 << 26;
        const STIBP                 = 1 << 27;
        const FLUSH_L1D             = 1 << 28;
        const ARCH_CAPABILITIES     = 1 << 29;
        const CORE_CAPABILITY       = 1 << 30;
        const SPEC_CTRL_SSBD        = 1 << 31;
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX7_ECX1 {
    pub eax: EAX7_ECX1_EAX,
    pub edx: EAX7_ECX1_EDX,
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX7_ECX1_EAX: u32 {
        const AVX_VNNI      = 1 << 4;
        const AVX512_BF16   = 1 << 5;
        const CMPCCXAD      = 1 << 7;
        const FZRM          = 1 << 10;
        const FSRS          = 1 << 11;
        const FSRC          = 1 << 12;
        const AMX_FP16      = 1 << 21;
        const AVX_IFMA      = 1 << 23;
    }
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX7_ECX1_EDX: u32 {
        const AVX_VNNI_INT8     = 1 << 4;
        const AVX_NE_CONVERT    = 1 << 5;
        const AMX_COMPLEX       = 1 << 8;
        const PREFETCHITI       = 1 << 14;
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX7_ECX2 {
    pub edx: EAX7_ECX2_EDX,
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX7_ECX2_EDX: u32 {
        const PSFD          = 1 << 0;
        const IPRED_CTRL    = 1 << 1;
        const RRSBA_CTRL    = 1 << 2;
        const DDPD_U        = 1 << 3;
        const BHI_CTRL      = 1 << 4;
        const MCDT_NO       = 1 << 5;
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX0D {
    pub ecx1: EAX0D_ECX1,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX0D_ECX1 {
    pub eax: EAX0D_ECX1_EAX,
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX0D_ECX1_EAX: u32 {
        const XSAVEOPT  = 1 << 0;
        const XSAVEC    = 1 << 1;
        const XGETBV1   = 1 << 2;
        const XSS       = 1 << 3;
        const XFD       = 1 << 4;
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct EAX8000_0001 {
    pub edx: EAX8000_0001_EDX,
    pub ecx: EAX8000_0001_ECX,
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX8000_0001_EDX: u32 {
        const FPU               = 1 << 0;
        const VME               = 1 << 1;
        const DE                = 1 << 2;
        const PSE               = 1 << 3;
        const TSC               = 1 << 4;
        const MSR               = 1 << 5;
        const PAE               = 1 << 6;
        const MCE               = 1 << 7;
        const CX8               = 1 << 8;
        const APIC              = 1 << 9;
        const SYSCALL           = 1 << 11;
        const MTRR              = 1 << 12;
        const PGE               = 1 << 13;
        const MCA               = 1 << 14;
        const CMOV              = 1 << 15;
        const PAT               = 1 << 16;
        const PSE36             = 1 << 17;
        const MP                = 1 << 19;
        const NX                = 1 << 20;
        const MMXEXT            = 1 << 22;
        const MMX               = 1 << 23;
        const FXSR              = 1 << 24;
        const FFXSR             = 1 << 25;
        const PDPE1GB           = 1 << 26;
        const RDTSCP            = 1 << 27;
        const LM                = 1 << 29;
        const CPUID_3DNOWEXT    = 1 << 30;
        const CPUID_3DNOW       = 1 << 31;
    }
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct EAX8000_0001_ECX: u32 {
        const LAHF_LM               = 1 << 0;
        const CMP_LEG               = 1 << 1;
        const SVM                   = 1 << 2;
        const EXTAPIC               = 1 << 3;
        const CR8LEG                = 1 << 4;
        const ABM                   = 1 << 5;
        const SSE4A                 = 1 << 6;
        const MISALIGNSSE           = 1 << 7;
        const CPUID_3DNOWPREFETCH   = 1 << 8;
        const OSVW                  = 1 << 9;
        const IBS                   = 1 << 10;
        const XOP                   = 1 << 11;
        const SKINIT                = 1 << 12;
        const WDT                   = 1 << 13;
        const LWP                   = 1 << 15;
        const FMA4                  = 1 << 16;
        const TCE                   = 1 << 17;
        const NODEID                = 1 << 19;
        const TBM                   = 1 << 21;
        const TOPOEXT               = 1 << 22;
        const PERFCORE              = 1 << 23;
        const PERFNB                = 1 << 24;
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct CpuFeatures {
    pub eax1: Option<EAX1>,
    pub eax6: Option<EAX6>,
    pub eax7: Option<EAX7>,
    pub eax0d: Option<EAX0D>,
    pub eax8000_0001: Option<EAX8000_0001>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CpuId {
    #[serde(serialize_with = "serialize_ascii", deserialize_with = "deserialize_name")]
    pub name: Vec<u8>,
    pub level: u32,
    #[serde(serialize_with = "serialize_ascii", deserialize_with = "deserialize_vendor")]
    pub vendor: Vec<u8>,
    pub family: u32,
    pub model: u32,
    pub stepping: u32,
    pub xlevel: u32,
    #[serde(serialize_with = "serialize_ascii", deserialize_with = "deserialize_model_id")]
    pub model_id: Vec<u8>,
    pub features: CpuFeatures,
}

fn serialize_ascii<S>(
    data: &Vec<u8>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    Serialize::serialize(&String::from_utf8(data.clone()).map_err(S::Error::custom)?, serializer)
}

fn deserialize_vendor<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let mut s = Vec::from(s);
    if s.len() > 12 {
        Err(D::Error::invalid_length(s.len(), &(12.to_string().as_str())))
    } else {
        s.resize(12, 0);
        Ok(s)
    }
}

fn deserialize_name<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let mut s = Vec::from(s);
    if s.len() > 12 {
        Err(D::Error::invalid_length(s.len(), &(12.to_string().as_str())))
    } else {
        s.resize(12, 0);
        Ok(s)
    }
}

fn deserialize_model_id<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let mut s = Vec::from(s);
    if s.len() > 48 {
        Err(D::Error::invalid_length(s.len(), &(12.to_string().as_str())))
    } else {
        s.resize(48, 0);
        Ok(s)
    }
}

impl CpuId {
    pub fn unknown() -> Self {
        Self {
            name: Vec::from("MOGIMOGIMOGI"),
            level: 0,
            vendor: Vec::from("MOGIMOGIMOGI"),
            family: 0,
            model: 0,
            stepping: 0,
            xlevel: 0,
            model_id: Vec::from("MOGIMOGIMOGI"),
            features: CpuFeatures {
                eax1: None,
                eax6: None,
                eax7: None,
                eax0d: None,
                eax8000_0001: None,
            },
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, CpuError> {
        json5::from_str::<CpuId>(
            std::fs::read_to_string(path)
                .map_err(|_| CpuError::Unknown)?
                .as_str(),
        )
        .map_err(|_| CpuError::Unknown)
    }
}
