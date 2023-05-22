use bitflags::bitflags;
use byteorder::{ByteOrder, LittleEndian};
use enumset::EnumSet;
use crate::emulator::abstractions::cpu::address::EVAddr;

use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::kernel::LinuxKernelError;
use crate::emulator::platform::linux::signals::{Signal, SignalAction};

pub enum X86 {}

bitflags! {
    pub struct X86SignalFlags: u64 {
        const SA_NOCLDSTOP = 1;
        const SA_NOCLDWAIT = 2;
        const SA_SIGINFO = 4;
        const SA_UNSUPPORTED = 0x400;
        const SA_EXPOSE_TAGBITS = 0x800;
        const SA_X32_ABI = 0x1000000;
        const SA_IA32_ABI = 0x2000000;
        const SA_ONSTACK = 0x8000000;
        const SA_RESTART = 0x10000000;
        const SA_NODEFER = 0x40000000;
        const SA_RESETHAND = 0x80000000;
        const SA_INTERRUPT = 0x20000000;
    }
}

impl Architecture for X86 {
    type SignalFlags = X86SignalFlags;

    fn read_signal(signum: i32) -> Result<Signal, LinuxKernelError> {
        match signum {
            1 => Ok(Signal::SIGHUP),
            2 => Ok(Signal::SIGINT),
            _ => Err(LinuxKernelError::EINVAL),
        }
    }
    fn read_sigaction(bytes: &[u8]) -> SignalAction<Self::SignalFlags> {
        // TODO: Handle invalid access beforehand and return error in trait method
        let sigaction = EVAddr::from(LittleEndian::read_u64(&bytes[0..8]));
        let flags = LittleEndian::read_u64(&bytes[8..16]);
        let restorer = EVAddr::from(LittleEndian::read_u64(&bytes[16..24]));
        let mask = LittleEndian::read_u64(&bytes[24..32]);
        SignalAction::new(sigaction, Some(restorer), mask, X86SignalFlags::from_bits_truncate(flags))
    }
}
