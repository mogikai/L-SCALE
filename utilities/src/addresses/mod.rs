use crate as mogi_utilities;
use crate::addresses::r#impl::addr_impl;
use crate::encoding::RequiredMem;
use std::ptr::NonNull;

mod r#impl;

/// Emulated virtual address
addr_impl!(EVAddr("V"));

/// Emulated physical address
addr_impl!(EPAddr("P"));

/// Host virtual address
pub type HVAddr<T> = NonNull<T>;
