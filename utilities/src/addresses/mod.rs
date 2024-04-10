use crate as mogi_utilities;
use crate::addresses::r#impl::addr_impl;
use crate::encoding::RequiredMem;

mod r#impl;

/// Emulated virtual address
addr_impl!(EVAddr);

/// Emulated physical address
addr_impl!(EPAddr);

/// Host virtual address
addr_impl!(HVAddr);
