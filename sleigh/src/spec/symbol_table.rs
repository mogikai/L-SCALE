use std::marker::PhantomData;

#[derive(Debug, Default)]
pub struct MOIRSymbolTable<K, V>(Vec<V>, PhantomData<K>);

impl<K, V> MOIRSymbolTable<K, V> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<K, V> MOIRSymbolTable<K, V>
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

impl<K, V> MOIRSymbolTable<K, V>
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
