use crate::emulator::abstractions::restore::Restorable;
use std::fmt::Debug;

pub trait TCPSystem: Restorable + Debug {}
