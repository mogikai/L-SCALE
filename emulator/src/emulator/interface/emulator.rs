use mogi_system::components::restore::Restorable;
use mogi_system::errors::EmulatorError;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::emulator::abstractions::system::SystemControl;
use crate::emulator::interface::meter::{
    DebuggableMeter,
    Meter,
};

#[derive(Debug)]
pub enum ExitReason {
    /// There are no more tasks that could be resumed (all processes exited)
    Stopped,
    /// All processes are in a blocked state and waiting for I/O
    Yielded,
    Breakpoint,
}

pub trait Emulator: Restorable {
    /// Runs the emulation until either the duration specified in `timeout` has elapsed,
    /// or for the number of instructions specified in `instruction_count`.
    /// If both are None, the emulation will run, until all tasks yield to the kernel.
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<ExitReason, EmulatorError>;

    /// Gives a reference to the system control pointers.
    /// The pointer collection can be cloned if needed.
    /// Cloning encompasses a clone of every system control
    /// pointer and shouldn't be that expensive.
    ///
    /// Keep in mind that the system control components should
    /// not be borrowed across other emulator calls, since the
    /// emulator will need to access them.
    fn system(&self) -> &impl SystemControl;

    /// Enables trace output for each cpu instruction that is executed.
    /// This is introduces a very large overhead, so only use it when
    /// absolutely necessary. It will also most likely quickly flood
    /// your terminal, so it is recommended to pipe the output into a
    /// file for later analysis.
    fn enable_trace(&mut self) -> Result<(), EmulatorError>;

    fn register_meter(
        &mut self,
        meter: Rc<RefCell<dyn DebuggableMeter>>,
    );
}
