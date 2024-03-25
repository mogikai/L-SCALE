macro_rules! addr_impl {
    ($(#[$outer:meta])* $name:ident) => {
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        #[derive(
            Hash,
            Ord,
            PartialOrd,
            Eq,
            PartialEq,
            Copy,
            Clone,
            Default,
            Add,
            Into,
            Not,
            UpperHex,
            BitAnd,
            Decode,
            Encode,
            RequiredMem,
        )]
        pub struct $name(u64);

        impl $name {
            pub fn null() -> Self {
                Self(0u64)
            }

            /// Checks if the address is null.
            /// returns: true, if the address is null and false otherwise
            pub fn is_null(&self) -> bool {
                self.0 == 0
            }
        }

        impl From<u16> for $name {
            fn from(value: u16) -> Self {
                Self(value.into())
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value.into())
            }
        }

        impl Debug for $name {
            fn fmt(
                &self,
                f: &mut Formatter<'_>,
            ) -> std::fmt::Result {
                write!(f, "v{:#018x?}", self.0)
            }
        }

        impl Sub<usize> for $name {
            type Output = $name;

            fn sub(
                self,
                rhs: usize,
            ) -> Self::Output {
                Self(self.0 - u64::try_from(rhs).expect("usize must be equal to  u64"))
            }
        }

        impl Add<usize> for $name {
            type Output = $name;

            fn add(
                self,
                rhs: usize,
            ) -> Self::Output {
                Self(self.0 + u64::try_from(rhs).expect("usize must be equal length to u64"))
            }
        }

        impl AddAssign<usize> for $name {
            fn add_assign(
                &mut self,
                rhs: usize,
            ) {
                self.0 += u64::try_from(rhs).expect("usize must be equal length to u64");
            }
        }

        impl Add<usize> for &$name {
            type Output = $name;

            fn add(
                self,
                rhs: usize,
            ) -> Self::Output {
                $name(self.0 + u64::try_from(rhs).expect("usize must be equal length to u64"))
            }
        }

        impl Add<u64> for $name {
            type Output = $name;

            fn add(
                self,
                rhs: u64,
            ) -> Self::Output {
                Self(self.0 + rhs)
            }
        }

        impl Add<u64> for &$name {
            type Output = $name;

            fn add(
                self,
                rhs: u64,
            ) -> Self::Output {
                $name(self.0 + rhs)
            }
        }

        impl Add<i32> for $name {
            type Output = $name;

            fn add(
                self,
                rhs: i32,
            ) -> Self::Output {
                Self(self.0.wrapping_add_signed(rhs.into()))
            }
        }

        impl Add<i32> for &$name {
            type Output = $name;

            fn add(
                self,
                rhs: i32,
            ) -> Self::Output {
                $name(self.0.wrapping_add_signed(rhs.into()))
            }
        }

        impl Sub<$name> for $name {
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
    };
}

pub(super) use addr_impl;
