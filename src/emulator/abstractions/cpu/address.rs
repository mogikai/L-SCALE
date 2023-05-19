use derive_more::{Add, From, Into};
/// Emulated virtual address
// #[derive(Eq, PartialEq)]
pub type EVAddr = u64;

/// Emulated physical address
#[derive(Eq, PartialEq, Copy, Clone, Add, From, Into)]
pub struct EPAddr(u64);
