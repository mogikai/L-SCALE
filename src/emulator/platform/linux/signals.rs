use std::fmt::Debug;
use std::collections::HashSet;

use enumset::EnumSet;
use enumset::EnumSetType;

use crate::emulator::abstractions::cpu::Vaddr;
use crate::emulator::platform::linux::kernel::LinuxKernelError;

#[derive(Debug, Clone, Copy)]
pub struct SignalAction<Flags: Debug + Clone + Copy> {
    handler: Vaddr,
    restorer: Option<Vaddr>,
    mask: u64,
    flags: Flags,
}

impl<Flags: Debug + Clone + Copy> SignalAction<Flags> {
    pub fn new(callback: Vaddr, restorer: Option<Vaddr>, mask: u64, flags: Flags) -> Self {
        return SignalAction{handler: callback, restorer: restorer, mask: mask, flags: flags};
    }
}

pub struct SignalHandler<Flags: Debug + Clone + Copy> {
    signals: [Option<SignalAction<Flags>>; 64],
    mask: u64,
}

impl<Flags: Debug + Clone + Copy> SignalHandler<Flags> {
    pub fn new() -> Self {
        return Self{ signals: [None; 64], mask: 0 };
    }

    pub fn insert(&mut self, signal: u8, handler: SignalAction<Flags>) {
        if signal >= 64 {
            panic!("no sutch signal");
        }
        self.signals[signal as usize] = Some(handler);
    }
    pub fn get(&self, signal: u8) -> Option<SignalAction<Flags>> {
        if signal >= 64 {
            return None
        }
        if 1 << signal & self.mask != 0 {
            return None;
        }
        println!("insert signalhandler at signal 0x{:X}", signal);
        return self.signals[signal as usize];
    }
    pub fn setmask(&mut self, how: SignalMaskFlags, mask: u64) {
        match how {
            SignalMaskFlags::SIG_BLOCK => self.mask |= mask,
            SignalMaskFlags::SIG_UNBLOCK => self.mask &= !mask,
            SignalMaskFlags::SIG_SET => self.mask = mask,
        }
    }
    pub fn getmask(&self) -> u64 {
        return self.mask;
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SignalMaskFlags {
    SIG_BLOCK,
    SIG_UNBLOCK,
    SIG_SET,
}

impl TryFrom<u64> for SignalMaskFlags {
    type Error = LinuxKernelError;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => return Ok(Self::SIG_BLOCK),
            1 => return Ok(Self::SIG_UNBLOCK),
            2 => return Ok(Self::SIG_SET),
            _ => return Err(LinuxKernelError::EINVAL),
        }
    }
}

#[derive(Debug, EnumSetType)]
pub enum Signal {
    SIGHUP,
    SIGINT,
    SIGQUIT,
    SIGILL,
    SIGTRAP,
    SIGABRT,
    SIGIOT,
    SIGBUS,
    SIGEMT,
    SIGFPE,
    SIGKILL,
    SIGUSR1,
    SIGSEGV,
    SIGUSR2,
    SIGPIPE,
    SIGALRM,
    SIGTERM,
    SIGSTKFLT,
    SIGCHLD,
    SIGCLD,
    SIGCONT,
    SIGSTOP,
    SIGTSTP,
    SIGTTIN,
    SIGTTOU,
    SIGURG,
    SIGXCPU,
    SIGXFSZ,
    SIGVTALRM,
    SIGPROF,
    SIGWINCH,
    SIGIO,
    SIGPOLL,
    SIGPWR,
    SIGINFO,
    SIGLOST,
    SIGSYS,
    SIGUNUSED,
}

#[derive(Debug, EnumSetType)]
pub enum GenericSignalFlag {
    SA_NOCLDSTOP,
    SA_NOCLDWAIT,
    SA_NODEFER,
    SA_ONSTACK,
    SA_RESETHAND,
    SA_RESTORER,
    SA_SIGINFO,
}
