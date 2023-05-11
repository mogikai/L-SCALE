/// Emulated virtual address
// #[derive(Eq, PartialEq)]
pub type EVAddr = u64;

/// Emulated physical address
#[derive(Eq, PartialEq, Copy, Clone)]
pub struct EPAddr(u64);

impl From<EPAddr> for u64 {
    fn from(value: EPAddr) -> Self {
        value.0
    }
}

impl From<u64> for EPAddr {
    fn from(value: u64) -> Self {
        EPAddr(value)
    }
}
