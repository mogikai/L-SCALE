use std::marker::PhantomData;

/// Creates a new symbol table identifier that can be used to index a [TypedIndexTable].
#[macro_export(local_inner_macros)]
macro_rules! new_typed_index_table_id {
    ( $(#[$outer:meta])* $vis:vis struct $name:ident; ) => {
        $(#[$outer])*
        #[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
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
    };
}

#[derive(Debug)]
pub struct TypedIndexTable<K, V>(Vec<V>, PhantomData<K>);
impl<K, V> Default for TypedIndexTable<K, V> {
    fn default() -> Self {
        Self {
            0: vec![],
            1: Default::default(),
        }
    }
}

impl<K, V> TypedIndexTable<K, V> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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
}
