#[cfg(feature = "serde")]
use serde::{
    Deserialize,
    Serialize,
};

pub mod lazy_clone_map;
pub mod restorable_map;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Hash, Eq, PartialEq, Copy, Clone, Debug, PartialOrd, Ord)]
pub struct AutoKey(usize);
