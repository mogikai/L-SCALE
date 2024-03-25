use crate as mogi_utilities;
use crate::encoding::*;
use std::marker::PhantomData;
use std::ops::Not;

struct X<T>(PhantomData<T>);

macro_rules! addr_impl {
    ($(#[$outer:meta])* $name:ident) => {
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(
            Hash,
            Ord,
            PartialOrd,
            Eq,
            PartialEq,
            Copy,
            Clone,
            Default,
            derive_more::Into,
            derive_more::UpperHex,
        )]
        #[rustfmt::skip]
        #[upper_hex("{}", _0)]
        pub struct $name<T = u8>(u64, std::marker::PhantomData::<T>);

        impl<T> crate::encoding::RequiredMem for $name<T> {}

        impl $name {
            pub fn null() -> Self {
                Self(0u64, Default::default())
            }

            /// Checks if the address is null.
            /// returns: true, if the address is null and false otherwise
            pub fn is_null(&self) -> bool {
                self.0 == 0
            }
        }

        impl From<u16> for $name {
            fn from(value: u16) -> Self {
                Self(value.into(), Default::default())
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value.into(), Default::default())
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(
                &self,
                f: &mut std::fmt::Formatter<'_>,
            ) -> std::fmt::Result {
                write!(f, "v{:#018x?}", self.0)
            }
        }

        impl std::ops::Not for $name {
            type Output = Self;
            fn not(self) -> Self::Output {
                Self(self.0.not(), Default::default())
            }
        }

        impl std::ops::BitAnd<Self> for $name {
            type Output = Self;
            fn bitand(
                self,
                rhs: Self,
            ) -> Self {
                Self(self.0.bitand(rhs.0), Default::default())
            }
        }

        impl std::ops::Sub<usize> for $name {
            type Output = $name;

            fn sub(
                self,
                rhs: usize,
            ) -> Self::Output {
                Self(self.0 - u64::try_from(rhs).expect("usize must be equal to  u64"), Default::default())
            }
        }

        impl std::ops::Add<usize> for $name {
            type Output = $name;

            fn add(
                self,
                rhs: usize,
            ) -> Self::Output {
                Self(self.0 + u64::try_from(rhs).expect("usize must be equal length to u64"), Default::default())
            }
        }

        impl std::ops::AddAssign<usize> for $name {
            fn add_assign(
                &mut self,
                rhs: usize,
            ) {
                self.0 += u64::try_from(rhs).expect("usize must be equal length to u64");
            }
        }

        impl std::ops::Add<usize> for &$name {
            type Output = $name;

            fn add(
                self,
                rhs: usize,
            ) -> Self::Output {
                $name(self.0 + u64::try_from(rhs).expect("usize must be equal length to u64"), Default::default())
            }
        }

        impl std::ops::Add<u64> for $name {
            type Output = $name;

            fn add(
                self,
                rhs: u64,
            ) -> Self::Output {
                Self(self.0 + rhs, Default::default())
            }
        }

        impl std::ops::Add<u64> for &$name {
            type Output = $name;

            fn add(
                self,
                rhs: u64,
            ) -> Self::Output {
                $name(self.0 + rhs, Default::default())
            }
        }

        impl std::ops::Add<i32> for $name {
            type Output = $name;

            fn add(
                self,
                rhs: i32,
            ) -> Self::Output {
                Self(self.0.wrapping_add_signed(rhs.into()), Default::default())
            }
        }

        impl std::ops::Add<i32> for &$name {
            type Output = $name;

            fn add(
                self,
                rhs: i32,
            ) -> Self::Output {
                $name(self.0.wrapping_add_signed(rhs.into()), Default::default())
            }
        }

        impl std::ops::Add<Self> for $name {
            type Output = $name;

            fn add(
                self,
                rhs: Self,
            ) -> Self::Output {
                Self(self.0 + rhs.0, Default::default())
            }
        }

        impl std::ops::Sub<$name> for $name {
            type Output = usize;

            fn sub(
                self,
                rhs: $name,
            ) -> Self::Output {
                usize::try_from(
                    self.0
                        .checked_sub(rhs.0)
                        .expect("Subtracting a larger address from a smaller address is undefined behavior"),
                )
                .expect("usize must be equal length to u64")
            }
        }

        impl crate::encoding::Encode for $name {
            fn encode<Endianness: byteorder::ByteOrder>(
                &self,
                buffer: &mut Vec<u8>,
            ) {
                self.0.encode::<Endianness>(buffer)
            }
        }

        impl crate::encoding::Decode for $name {
            fn decode<Endianness: byteorder::ByteOrder>(val: &[u8]) -> Result<Self, crate::error::memory::MemoryError> {
                Ok(Self(u64::decode::<Endianness>(val)?, Default::default()))
            }
        }
    };
}

pub(super) use addr_impl;
