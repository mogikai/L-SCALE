pub mod x86;

use std::fmt::Debug;

use crate::emulator::platform::linux::signals::{Signal, SignalAction};
use crate::emulator::platform::linux::kernel::LinuxKernelError;

pub trait Architecture {
    type SignalFlags: Debug + Clone + Copy;

    fn read_signal(signum: i32) -> Result<Signal, LinuxKernelError>;
    fn read_sigaction(bytes: &[u8]) -> SignalAction<Self::SignalFlags>;
}
