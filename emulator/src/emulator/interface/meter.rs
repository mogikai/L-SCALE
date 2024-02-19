use std::fmt::Debug;

/// Objects that implement this trait can be registered in the emulator as a meter.
/// The emulator will start any metering objects upon starting emulation, and will
/// then stop them, once the emulation finishes.
pub trait Meter {
    fn start(&mut self);
    fn stop(&mut self);
}

pub trait DebuggableMeter: Meter + Debug {}

impl<T> DebuggableMeter for T where T: Meter + Debug {}
