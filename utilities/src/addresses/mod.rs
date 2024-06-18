use crate as mogi_utilities;
use crate::addresses::r#impl::addr_impl;
use crate::encoding::RequiredMem;
use std::ptr::NonNull;

mod r#impl;

/// This trait can be implemented on objects that have an emulated virtual address.
pub trait EVAddress {
    fn ev_address(&self) -> EVAddr;
}

/// Emulated virtual address
addr_impl!(EVAddr("V"));

/// Emulated physical address
addr_impl!(EPAddr("P"));

/// Host virtual address
pub type HVAddr<T> = NonNull<T>;
