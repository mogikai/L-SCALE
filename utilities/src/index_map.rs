use std::hash::Hash;
use std::marker::PhantomData;
use std::num::NonZero;
use std::ops::Index;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexMapError {
    #[error("Trying to create an index from zero, which is not a valid index.")]
    ZeroIndex,
}

/// Marker trait for keys that can be used with a [PrimaryMap].
pub trait MapKey: Copy + PartialEq + Eq + Hash {}

#[macro_export(local_inner_macros)]
macro_rules! new_map_key {
    ( $(#[$outer:meta])* $vis:vis struct $name:ident; $display_prefix:literal;) => {
        $(#[$outer])*
        #[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
        #[repr(transparent)]
        $vis struct $name(std::num::NonZero<usize>);
        impl TryFrom<usize> for $name {
            type Error = $crate::index_map::IndexMapError;
            fn try_from(value: usize) -> Result<Self, Self::Error> {
                std::num::NonZero::new(value).ok_or($crate::index_map::IndexMapError::ZeroIndex).map(Self)
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0.get()
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(
                &self,
                f: &mut std::fmt::Formatter<'_>,
            ) -> std::fmt::Result {
                std::write!(f, "{}{}", $display_prefix, self.0)
            }
        }
        impl $crate::index_map::MapKey for $name {}
    };
}

#[derive(Debug)]
pub struct PrimaryMap<K: MapKey, V> {
    data: Vec<V>,
    phantom_data: PhantomData<K>,
}

impl<K: MapKey, V> Default for PrimaryMap<K, V> {
    fn default() -> Self {
        Self {
            data: Default::default(),
            phantom_data: Default::default(),
        }
    }
}

impl<K: MapKey, V: Clone> Clone for PrimaryMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            phantom_data: Default::default(),
        }
    }
}

impl<K: MapKey, V> Index<K> for PrimaryMap<K, V> {
    type Output = Option<V>;

    fn index(
        &self,
        index: K,
    ) -> &Self::Output {
        todo!()
    }
}

impl<K: MapKey, V> PrimaryMap<K, V> {}
