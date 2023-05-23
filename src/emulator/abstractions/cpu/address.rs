use std::ops::{Add, AddAssign, Sub};
use derive_more::{Add, BitAnd, From, Into, Not, UpperHex};
/// Emulated virtual address
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Add, From, Into, Not, UpperHex, BitAnd)]
pub struct EVAddr(u64);

impl Sub<usize> for EVAddr {
    type Output = EVAddr;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - u64::try_from(rhs).expect("usize must be equal to  u64"))
    }
}

impl Add<usize> for EVAddr {
    type Output = EVAddr;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + u64::try_from(rhs).expect("usize must be equal length to u64"))
    }
}

impl Add<usize> for &EVAddr {
    type Output = EVAddr;

    fn add(self, rhs: usize) -> Self::Output {
        EVAddr(self.0 + u64::try_from(rhs).expect("usize must be equal length to u64"))
    }
}

impl Add<u64> for EVAddr {
    type Output = EVAddr;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Add<u64> for &EVAddr {
    type Output = EVAddr;

    fn add(self, rhs: u64) -> Self::Output {
        EVAddr(self.0 + rhs)
    }
}

impl Add<i32> for EVAddr {
    type Output = EVAddr;

    fn add(self, rhs: i32) -> Self::Output {
        Self(self.0.wrapping_add_signed(rhs.into()))
    }
}

impl Add<i32> for &EVAddr {
    type Output = EVAddr;

    fn add(self, rhs: i32) -> Self::Output {
        EVAddr(self.0.wrapping_add_signed(rhs.into()))
    }
}

impl Sub<EVAddr> for EVAddr {
    type Output = usize;

    fn sub(self, rhs: EVAddr) -> Self::Output {
        usize::try_from(self.0 - rhs.0).expect("usize must be equal length to u64")
    }
}

pub const EVA_NULL: EVAddr = EVAddr(0);

/// Emulated physical address
#[derive(Eq, PartialEq, Copy, Clone, Add, From, Into)]
pub struct EPAddr(u64);

impl Sub<usize> for EPAddr {
    type Output = EPAddr;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - u64::try_from(rhs).expect("usize must be equal length to u64"))
    }
}

impl Add<usize> for EPAddr {
    type Output = EPAddr;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + u64::try_from(rhs).expect("usize must be equal length to u64"))
    }
}

impl AddAssign<usize> for EPAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += u64::try_from(rhs).expect("usize must be equal length to u64")
    }
}