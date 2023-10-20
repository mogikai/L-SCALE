pub mod lazy_clone_map;
pub mod restorable_map;

#[derive(Hash, Eq, PartialEq, Copy, Clone, Debug)]
pub struct AutoKey(usize);
