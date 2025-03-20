use std::fmt::Debug;

use crate::emulator::platform::linux::kernel::LinuxKernelError;
use crate::emulator::platform::linux::signals::{
    Signal,
    SignalAction,
};

pub mod x86;

pub trait Architecture {
    type SignalFlags: Debug + Clone + Copy;

    fn read_signal(signum: i32) -> Result<Signal, LinuxKernelError>;
    fn read_sigaction(bytes: &[u8]) -> SignalAction<Self::SignalFlags>;
}
