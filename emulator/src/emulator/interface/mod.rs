//! The interface module defines traits that define the functionality for the subsystems that can be accessed from
//! outside the emulator. In contrast, the `abstractions` module defines traits that are used by the emulator itself.
//! The externally facing interfaces can be used for example to control the tcp streams that the emulator interacts
//! with, or to analyze the memory of a process.
pub mod emulator;
pub mod error;
pub mod meter;
pub mod network;
