use crate::DecodeError;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
pub enum ExceptionCode {
    None                = 0x0000,

    InstructionLimit    = 0x0001,
    Halt                = 0x0002,
    Sleep               = 0x0003,
    SoftwareBreakpoint  = 0x0004,

    Syscall             = 0x0101,
    CpuStateChanged     = 0x0102,
    DivideByZero        = 0x0103,

    ReadUnmapped        = 0x0201,
    ReadPerm            = 0x0202,
    ReadUnaligned       = 0x0203,
    ReadWatch           = 0x0204,
    ReadUninitialized   = 0x0205,

    WriteUnmapped       = 0x0301,
    WritePerm           = 0x0302,
    WriteWatch          = 0x0303,
    WriteUnaligned      = 0x0304,

    ExecViolation       = 0x0401,
    SelfModifyingCode   = 0x0402,
    ExecUnaligned       = 0x0403,

    OutOfMemory         = 0x0501,
    AddressOverflow     = 0x0502,

    InvalidInstruction  = 0x1001,
    UnknownInterrupt    = 0x1002,
    UnknownCpuID        = 0x1003,
    InvalidOpSize       = 0x1004,
    InvalidFloatSize    = 0x1005,
    CodeNotTranslated   = 0x1006,
    ShadowStackOverflow = 0x1007,
    ShadowStackInvalid  = 0x1008,
    InvalidTarget       = 0x1009,
    UnimplementedOp     = 0x100a,

    ExternalAddr        = 0x2001,
    Environment         = 0x2002,

    JitError            = 0x3001,
    InternalError       = 0x3002,

    UnknownError,
}

impl ExceptionCode {
    #[inline]
    pub fn from_u32(code: u32) -> Self {
        match code {
            0x0000 => Self::None,
            0x0001 => Self::InstructionLimit,
            0x0002 => Self::Halt,
            0x0003 => Self::Sleep,
            0x0004 => Self::SoftwareBreakpoint,

            0x0101 => Self::Syscall,
            0x0102 => Self::CpuStateChanged,
            0x0103 => Self::DivideByZero,

            0x0201 => Self::ReadUnmapped,
            0x0202 => Self::ReadPerm,
            0x0203 => Self::ReadUnaligned,
            0x0204 => Self::ReadWatch,
            0x0205 => Self::ReadUninitialized,

            0x0301 => Self::WriteUnmapped,
            0x0302 => Self::WritePerm,
            0x0303 => Self::WriteWatch,
            0x0304 => Self::WriteUnaligned,

            0x0401 => Self::ExecViolation,
            0x0402 => Self::SelfModifyingCode,
            0x0403 => Self::ExecUnaligned,

            0x0501 => Self::OutOfMemory,
            0x0502 => Self::AddressOverflow,

            0x1001 => Self::InvalidInstruction,
            0x1002 => Self::UnknownInterrupt,
            0x1003 => Self::UnknownCpuID,
            0x1004 => Self::InvalidOpSize,
            0x1005 => Self::InvalidFloatSize,
            0x1006 => Self::CodeNotTranslated,
            0x1007 => Self::ShadowStackOverflow,
            0x1008 => Self::ShadowStackInvalid,
            0x1009 => Self::InvalidTarget,
            0x100a => Self::UnimplementedOp,

            0x2001 => Self::ExternalAddr,
            0x2002 => Self::Environment,

            0x3001 => Self::JitError,
            0x3002 => Self::InternalError,

            _ => {
                if cfg!(debug_assertions) {
                    panic!("Unknown exception code: {:#0x}", code);
                }
                Self::UnknownError
            }
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::None | Self::InstructionLimit)
    }

    pub fn is_memory_error(&self) -> bool {
        matches!(
            self,
            Self::ReadUnmapped
                | Self::ReadPerm
                | Self::ReadUnaligned
                | Self::ReadWatch
                | Self::ReadUninitialized
                | Self::WriteUnmapped
                | Self::WritePerm
                | Self::WriteWatch
                | Self::WriteUnaligned
                | Self::SelfModifyingCode
        )
    }

    pub fn from_load_error(err: glaze_mem::MemError) -> Self {
        use glaze_mem::MemError;
        match err {
            MemError::Unmapped => Self::ReadUnmapped,
            MemError::Uninitalized => Self::ReadUninitialized,
            MemError::ReadViolation => Self::ReadPerm,
            MemError::Unaligned => Self::ReadUnaligned,
            MemError::ReadWatch => Self::ReadWatch,
            _ => Self::from(err),
        }
    }

    pub fn from_store_error(err: glaze_mem::MemError) -> Self {
        use glaze_mem::MemError;
        match err {
            MemError::Unmapped => Self::WriteUnmapped,
            MemError::WriteViolation => Self::WritePerm,
            MemError::Unaligned => Self::WriteUnaligned,
            MemError::WriteWatch => Self::WriteWatch,
            _ => Self::from(err),
        }
    }
}

impl From<glaze_mem::MemError> for ExceptionCode {
    fn from(err: glaze_mem::MemError) -> Self {
        use glaze_mem::MemError;
        match err {
            MemError::Unmapped => Self::ReadUnmapped,
            MemError::Uninitalized => Self::ReadUninitialized,
            MemError::ReadViolation => Self::ReadPerm,
            MemError::WriteViolation => Self::WritePerm,
            MemError::ExecViolation => Self::ExecViolation,
            MemError::ReadWatch => Self::ReadWatch,
            MemError::WriteWatch => Self::WriteWatch,
            MemError::Unaligned => Self::ReadUnaligned,
            MemError::OutOfMemory => Self::OutOfMemory,
            MemError::SelfModifyingCode => Self::SelfModifyingCode,
            MemError::AddressOverflow => Self::AddressOverflow,

            // These are errors that should be handled by the memory subsystem.
            MemError::Unallocated | MemError::Unknown => Self::UnknownError,
        }
    }
}

impl From<DecodeError> for ExceptionCode {
    fn from(err: DecodeError) -> Self {
        match err {
            DecodeError::InvalidInstruction => ExceptionCode::InvalidInstruction,
            DecodeError::NonExecutableMemory => ExceptionCode::ExecViolation,
            DecodeError::BadAlignment => ExceptionCode::ExecUnaligned,
            DecodeError::DisassemblyChanged => ExceptionCode::SelfModifyingCode,
            DecodeError::OptimizationError => ExceptionCode::UnknownError,
        }
    }
}
