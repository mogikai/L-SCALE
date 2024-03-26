use crate as mogi_utilities;
use crate::addresses::r#impl::addr_impl;

mod r#impl;

// TODO FIXME: Make address calculations use the correct offsets, depending on type T

/// Emulated virtual address
addr_impl!(EVAddr);

/// Emulated physical address
addr_impl!(EPAddr);

/// Host virtual address
addr_impl!(HVAddr);
