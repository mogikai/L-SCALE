use serde_derive::{
    Deserialize,
    Serialize,
};
use std::marker::PhantomData;

/// Creates a new symbol table identifier that can be used to index a [TypedIndexTable].
#[macro_export(local_inner_macros)]
macro_rules! new_typed_index_table_id {
    ( $(#[$outer:meta])* $vis:vis struct $name:ident; $display_prefix:literal;) => {
        $(#[$outer])*
        #[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Copy, Clone, Hash, Default)]
        #[derive(serde::Serialize, serde::Deserialize)]
        #[repr(transparent)]
        $vis struct $name(usize);
        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(value)
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0
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
    };
}

/// Creates a new symbol table identifier that can be used to index a [TypedIndexTable].
/// The backing type is a 16 bit integer
#[macro_export(local_inner_macros)]
macro_rules! new_typed_index_table_id_16 {
    ( $(#[$outer:meta])* $vis:vis struct $name:ident; $display_prefix:literal;) => {
        $(#[$outer])*
        #[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
        #[derive(serde::Serialize, serde::Deserialize)]
        #[repr(transparent)]
        $vis struct $name(u16);
        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(value.try_into().expect("16 bit keys must be in the range [0, 65535]"))
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0.into()
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
    };
}

/// Creates a new symbol table identifier that can be used to index a [TypedIndexTable].
/// The backing type is a 16 bit integer
#[macro_export(local_inner_macros)]
macro_rules! new_typed_index_table_id_8 {
    ( $(#[$outer:meta])* $vis:vis struct $name:ident; $display_prefix:literal;) => {
        $(#[$outer])*
        #[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
        #[derive(serde::Serialize, serde::Deserialize)]
        #[repr(transparent)]
        $vis struct $name(u8);
        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(value.try_into().expect(std::format!("8 bit keys must be in the range [0, 255]. Got {}", value).as_str()))
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0.into()
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
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedIndexTable<K, V>(Vec<V>, PhantomData<K>);
impl<K, V> Default for TypedIndexTable<K, V> {
    fn default() -> Self {
        Self(vec![], Default::default())
    }
}

impl<K, V> FromIterator<(K, V)> for TypedIndexTable<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self(Vec::from_iter(iter.into_iter().map(|(_, v)| v)), Default::default())
    }
}

impl<K, V> Extend<(K, V)> for TypedIndexTable<K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(
        &mut self,
        iter: T,
    ) {
        self.0.extend(iter.into_iter().map(|(_, v)| v));
    }
}

impl<K, V> TypedIndexTable<K, V> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }
}

impl<K, V> TypedIndexTable<K, V>
where
    K: From<usize>,
{
    pub fn insert(
        &mut self,
        element: V,
    ) -> K {
        self.0.push(element);
        K::from(self.0.len() - 1)
    }

    pub fn next_key(&self) -> K {
        K::from(self.0.len())
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0.iter()
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.0.iter_mut()
    }

    pub fn into_values(self) -> impl Iterator<Item = V> {
        self.0.into_iter()
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.0.iter().enumerate().map(|(i, v)| (K::from(i), v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.0.iter_mut().enumerate().map(|(i, v)| (K::from(i), v))
    }

    pub fn into_iter(self) -> impl Iterator<Item = (K, V)> {
        self.0.into_iter().enumerate().map(|(i, v)| (K::from(i), v))
    }
}

impl<K, V> TypedIndexTable<K, V>
where
    usize: From<K>,
{
    pub fn get(
        &self,
        key: K,
    ) -> Option<&V> {
        self.0.get(usize::from(key))
    }

    pub fn get_mut(
        &mut self,
        key: K,
    ) -> Option<&mut V> {
        self.0.get_mut(usize::from(key))
    }

    pub fn replace(
        &mut self,
        key: K,
        element: V,
    ) -> Option<V> {
        self.0
            .get_mut(usize::from(key)).map(|prev| std::mem::replace(prev, element))
    }
}
