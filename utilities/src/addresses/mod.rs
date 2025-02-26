use crate::addresses::r#impl::addr_impl;
use crate::encoding::RequiredMem;
use std::ptr::NonNull;

mod r#impl;

/// A marker trait for the possible address types.
pub trait Address {}

/// This trait can be implemented on objects that have an address.
/// The type of the address is specified in the generic parameter `T`.
#[allow(private_bounds)]
pub trait Addressable<T>
where
    T: Address,
{
    fn address(&self) -> T;
}

addr_impl! {
    /// Emulated virtual address
    EVAddr("V")
}

addr_impl! {
    /// Emulated physical address
    EPAddr("P")
}

/// Host virtual address
pub type HVAddr<T> = NonNull<T>;
impl<T> Address for HVAddr<T> {}
