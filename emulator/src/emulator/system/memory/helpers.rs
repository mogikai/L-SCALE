use std::ops::{Add, BitAnd, Not};
use std::fmt::Debug;

const MASK4K: u64 = 4 * (1 << 10) - 1;

pub(crate) fn align4k_up<T>(address: T) -> T
    where
        T: Add<T, Output=T> + Not<Output=T> + TryFrom<u64> + BitAnd<Output=T> + Copy,
        <T as TryFrom<u64>>::Error: Debug,
{
    let mask = T::try_from(MASK4K).expect("Address like type must be convertible from 64 bit values");
    (address + mask) & !mask
}

pub(crate) fn align4k_down<T>(address: T) -> T
    where
        T: Not<Output=T> + TryFrom<u64> + BitAnd<Output=T>,
        <T as TryFrom<u64>>::Error: Debug,
{
    address & !T::try_from(MASK4K).expect("Address like type must be convertible from 64 bit values")
}

pub(crate) fn is4kaligned<T>(address: T) -> bool
    where
        T: TryFrom<u64> + BitAnd<Output=T> + Eq + Copy,
        <T as TryFrom<u64>>::Error: Debug,
{
    let mask = T::try_from(MASK4K).expect("Address like type must be convertible from 64 bit values");
    (address & mask) == T::try_from(0u64).expect("Address like type must be convertible from 64 bit values")
}
