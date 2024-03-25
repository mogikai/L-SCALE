use std::fmt::{
    Debug,
    Formatter,
};
use std::ops::{
    AddAssign,
    Sub,
};

use derive_more::{
    Add,
    BitAnd,
    Into,
    Not,
    UpperHex,
};
#[cfg(feature = "serde")]
use serde::{
    Deserialize,
    Serialize,
};

use mogi_derive::{
    Decode,
    Encode,
    RequiredMem,
};

use crate as mogi_utilities;
use crate::addresses::r#impl::addr_impl;

mod r#impl;

// TODO: Maybe it makes sense to parametrize the addresses by T, such that pointers to different types cannot be confused?

/// Emulated virtual address
addr_impl!(EVAddr);

/// Emulated physical address
addr_impl!(EPAddr);

/// Host virtual address
addr_impl!(HVAddr);
