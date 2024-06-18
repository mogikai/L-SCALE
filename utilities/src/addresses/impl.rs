use crate as mogi_utilities;
use crate::encoding::RequiredMem;

macro_rules! addr_impl {
    ($(#[$outer:meta])* $name:ident($debug:literal)) => {
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(
            Hash,
            Ord,
            PartialOrd,
            Eq,
            PartialEq,
            Default,
            derive_more::Into,
            derive_more::UpperHex,
        )]
        #[rustfmt::skip]
        #[upper_hex("{}", _0)]
        #[repr(transparent)]
        pub struct $name<T = u8>(u64, std::marker::PhantomData::<T>);

        impl<T> Copy for $name<T> {}

        impl<T> Clone for $name<T> {
            fn clone(&self) -> Self {
                Self(self.0, Default::default())
            }
        }

        impl<T> crate::encoding::RequiredMem for $name<T> {}

        impl<T> $name<T> {
            pub fn null() -> Self {
                Self(0u64, Default::default())
            }

            /// Checks if the address is null.
            /// returns: true, if the address is null and false otherwise
            pub fn is_null(&self) -> bool {
                self.0 == 0
            }

            pub unsafe fn transmute<U>(self) -> $name<U> {
                $name::<U>(self.0, Default::default())
            }

            pub const fn bits() -> usize {
                // TODO: Make backing type of the address generic
                std::mem::size_of::<u64>() * 8
            }
        }

        impl<T> From<u16> for $name<T> {
            fn from(value: u16) -> Self {
                Self(value.into(), Default::default())
            }
        }

        impl<T> From<u64> for $name<T> {
            fn from(value: u64) -> Self {
                Self(value.into(), Default::default())
            }
        }

        impl<T> From<$name<T>> for u64 {
            fn from(value: $name<T>) -> Self {
                value.0
            }
        }

        impl<T> From<$name<T>> for usize {
            fn from(value: $name<T>) -> Self {
                value.0 as usize
            }
        }

        impl<T> std::fmt::Debug for $name<T> {
            fn fmt(
                &self,
                f: &mut std::fmt::Formatter<'_>,
            ) -> std::fmt::Result {
                write!(f, "{}{:#018x?}", $debug, self.0)
            }
        }

        impl<T, U> std::ops::Shr<U> for $name<T>
        where
            u64: std::ops::Shr<U, Output = u64>,
        {
            type Output = Self;
            fn shr(
                self,
                rhs: U,
            ) -> Self::Output {
                Self(self.0 >> rhs, Default::default())
            }
        }

        impl<T, U> std::ops::Shr<U> for &$name<T>
        where
            u64: std::ops::Shr<U, Output = u64>,
        {
            type Output = $name<T>;
            fn shr(
                self,
                rhs: U,
            ) -> Self::Output {
                *self >> rhs
            }
        }

        impl<T, U> std::ops::Shl<U> for $name<T>
        where
            u64: std::ops::Shl<U, Output = u64>,
        {
            type Output = Self;
            fn shl(
                self,
                rhs: U,
            ) -> Self::Output {
                Self(self.0 << rhs, Default::default())
            }
        }

        impl<T, U> std::ops::Shl<U> for &$name<T>
        where
            u64: std::ops::Shl<U, Output = u64>,
        {
            type Output = $name<T>;
            fn shl(
                self,
                rhs: U,
            ) -> Self::Output {
                *self << rhs
            }
        }

        impl<T> std::ops::Not for $name<T> {
            type Output = Self;
            fn not(self) -> Self::Output {
                Self(self.0.not(), Default::default())
            }
        }

        impl<T> std::ops::BitAnd<Self> for $name<T> {
            type Output = Self;
            fn bitand(
                self,
                rhs: Self,
            ) -> Self {
                Self(self.0.bitand(rhs.0), Default::default())
            }
        }

        impl<T, U> std::ops::BitAnd<U> for $name<T>
        where
            u64: std::ops::BitAnd<U, Output = u64>,
        {
            type Output = u64;
            fn bitand(
                self,
                rhs: U,
            ) -> u64 {
                self.0 & rhs
            }
        }

        impl<T> std::ops::Sub<usize> for $name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn sub(
                self,
                rhs: usize,
            ) -> Self::Output {
                Self(
                    self.0 - u64::try_from(rhs * T::required_mem()).expect("usize must be equal to  u64"),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::Add<usize> for $name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn add(
                self,
                rhs: usize,
            ) -> Self::Output {
                $name::<T>(
                    self.0 + u64::try_from(rhs * T::required_mem()).expect("usize must be equal length to u64"),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::AddAssign<usize> for $name<T>
        where
            T: RequiredMem,
        {
            fn add_assign(
                &mut self,
                rhs: usize,
            ) {
                self.0 += u64::try_from(rhs * T::required_mem()).expect("usize must be equal length to u64");
            }
        }

        impl<T> std::ops::AddAssign<u64> for $name<T>
        where
            T: RequiredMem,
        {
            fn add_assign(
                &mut self,
                rhs: u64,
            ) {
                self.0 += rhs;
            }
        }

        impl<T> std::ops::Add<usize> for &$name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn add(
                self,
                rhs: usize,
            ) -> Self::Output {
                $name(
                    self.0 + u64::try_from(rhs * T::required_mem()).expect("usize must be equal length to u64"),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::Add<u8> for $name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn add(
                self,
                rhs: u8,
            ) -> Self::Output {
                Self(
                    self.0
                        + u64::from(rhs) * u64::try_from(T::required_mem()).expect("usize must be equal length to u64"),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::Add<u64> for $name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn add(
                self,
                rhs: u64,
            ) -> Self::Output {
                Self(
                    self.0 + rhs * u64::try_from(T::required_mem()).expect("usize must be equal length to u64"),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::Add<u64> for &$name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn add(
                self,
                rhs: u64,
            ) -> Self::Output {
                $name(
                    self.0 + rhs * u64::try_from(T::required_mem()).expect("usize must be equal length to u64"),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::Add<i32> for $name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn add(
                self,
                rhs: i32,
            ) -> Self::Output {
                Self(
                    self.0.wrapping_add_signed(
                        i64::from(rhs)
                            * i64::try_from(T::required_mem()).expect("struct size cannot be larger than i64::max"),
                    ),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::Add<i32> for &$name<T>
        where
            T: RequiredMem,
        {
            type Output = $name<T>;

            fn add(
                self,
                rhs: i32,
            ) -> Self::Output {
                $name(
                    self.0.wrapping_add_signed(
                        i64::from(rhs)
                            * i64::try_from(T::required_mem()).expect("struct size cannot be larger than i64::max"),
                    ),
                    Default::default(),
                )
            }
        }

        impl<T> std::ops::Add<Self> for $name<T> {
            type Output = $name<T>;

            fn add(
                self,
                rhs: Self,
            ) -> Self::Output {
                Self(self.0 + rhs.0, Default::default())
            }
        }

        impl<T> std::ops::Sub<$name> for $name<T> {
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

        impl<T> crate::encoding::Encode for $name<T> {
            fn encode<Endianness: byteorder::ByteOrder>(
                &self,
                buffer: &mut Vec<u8>,
            ) {
                self.0.encode::<Endianness>(buffer)
            }
        }

        impl<T> crate::encoding::Decode for $name<T> {
            fn decode<Endianness: byteorder::ByteOrder>(val: &[u8]) -> Result<Self, crate::error::memory::MemoryError> {
                Ok(Self(u64::decode::<Endianness>(val)?, Default::default()))
            }
        }
    };
}

pub(super) use addr_impl;
