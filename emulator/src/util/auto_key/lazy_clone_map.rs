use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use imbl::HashMap;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::util::auto_key::AutoKey;
use crate::util::rcow::RCow;

#[derive(Debug)]
struct Snapshot<T>
where
    T: Clone,
{
    data: HashMap<AutoKey, RCow<T>>,
}

#[derive(Debug)]
pub struct LazyCloneMap<T>
where
    T: Clone,
{
    data: HashMap<AutoKey, RCow<T>>,
    next_key: Rc<RefCell<AutoKey>>,
}

impl<T> Clone for LazyCloneMap<T>
where
    T: Clone,
{
    /// The key is unique, even across cloned maps.
    fn clone(&self) -> Self {
        LazyCloneMap {
            data: self.data.clone(),
            next_key: self.next_key.clone(),
        }
    }
}

impl<T> Default for LazyCloneMap<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LazyCloneMap<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self {
            data: Default::default(),
            next_key: Rc::new(RefCell::new(AutoKey(0))),
        }
    }

    pub fn insert(
        &mut self,
        element: T,
    ) -> AutoKey {
        let mut key = self.next_key.borrow_mut();
        let old = self.data.insert(*key, RCow::new(element));
        // Due to our key always incrementing, there will never be an old element.
        assert!(old.is_none());
        let current_key = *key;
        *key = AutoKey(key.0.checked_add(1).expect("We exhausted 2^64 keys. How?"));
        current_key
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(
        &self,
        key: &AutoKey,
    ) -> Option<&T> {
        self.data.get(key).map(|v| v.deref())
    }

    pub fn get_mut(
        &mut self,
        key: &AutoKey,
    ) -> Option<&mut T> {
        self.data.get_mut(key).map(|v| v.deref_mut())
    }

    pub fn remove(
        &mut self,
        key: &AutoKey,
    ) {
        self.data.remove(key);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AutoKey, &T)> {
        self.data.iter().map(|(k, v)| (k, v.deref()))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&AutoKey, &mut T)> {
        self.data.iter_mut().map(|(k, v)| (k, v.deref_mut()))
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.data.values().map(|v| v.deref())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.data.iter_mut().map(|(k, v)| v.deref_mut())
    }

    pub fn clear(&mut self) {
        self.data.clear()
    }
}

// impl<T> Restorable for LazyCloneMap<T>
// where
//     T: Clone,
// {
//     fn snapshot(&mut self) -> Result<(), GenericError> {
//         self.snapshots.push(Snapshot {
//             data: self.data.clone(),
//         });
//         Ok(())
//     }
//
//     fn restore_and_discard(&mut self) -> Result<(), GenericError> {
//         self.restore()?;
//         self.snapshots.pop();
//         Ok(())
//     }
//
//     fn restore(&mut self) -> Result<(), GenericError> {
//         if let Some(snapshot) = self.snapshots.last() {
//             self.data = snapshot.data.clone();
//         } else {
//             self.data = Default::default();
//         }
//         Ok(())
//     }
// }
//
#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn restore_correclty_restores_data() {
    //     let mut map = LazyCloneMap::<u32>::new();
    //     let key = map.insert(42);
    //     let key2 = map.insert(1337);
    //     map.remove(&key2);
    //     assert_eq!(*map.get(&key).unwrap(), 42);
    //     assert!(map.get(&key2).is_none());
    //
    //     map.snapshot();
    //     let key3 = map.insert(123);
    //     let key4 = map.insert(1337);
    //     assert_eq!(*map.get(&key).unwrap(), 42);
    //     *map.get_mut(&key).unwrap() += 1;
    //     assert_eq!(*map.get(&key).unwrap(), 43);
    //     assert!(map.get(&key2).is_none());
    //     assert_eq!(*map.get(&key3).unwrap(), 123);
    //     assert_eq!(*map.get(&key4).unwrap(), 1337);
    //
    //     map.restore();
    //     assert_eq!(*map.get(&key).unwrap(), 42);
    //     assert!(map.get(&key2).is_none());
    //     assert!(map.get(&key3).is_none());
    //     assert!(map.get(&key4).is_none());
    //
    //     map.restore_and_discard();
    //     assert_eq!(*map.get(&key).unwrap(), 42);
    //     assert!(map.get(&key2).is_none());
    //     assert!(map.get(&key3).is_none());
    //     assert!(map.get(&key4).is_none());
    //     assert!(!map.is_empty());
    //     assert_eq!(map.len(), 1);
    //
    //     map.restore_and_discard();
    //     assert!(map.get(&key).is_none());
    //     assert!(map.get(&key2).is_none());
    //     assert!(map.get(&key3).is_none());
    //     assert!(map.get(&key4).is_none());
    //     assert!(map.is_empty());
    // }
    #[test]
    fn restore_correclty_restores_data() {
        let mut map = LazyCloneMap::<u32>::new();
        let key = map.insert(42);
        let key2 = map.insert(1337);
        map.remove(&key2);
        assert_eq!(*map.get(&key).unwrap(), 42);
        assert!(map.get(&key2).is_none());

        let saved_map = map.clone();
        let key3 = map.insert(123);
        let key4 = map.insert(1337);
        assert_eq!(*map.get(&key).unwrap(), 42);
        *map.get_mut(&key).unwrap() += 1;
        assert_eq!(*map.get(&key).unwrap(), 43);
        assert!(map.get(&key2).is_none());
        assert_eq!(*map.get(&key3).unwrap(), 123);
        assert_eq!(*map.get(&key4).unwrap(), 1337);

        map.clone_from(&saved_map);
        assert_eq!(*map.get(&key).unwrap(), 42);
        assert!(map.get(&key2).is_none());
        assert!(map.get(&key3).is_none());
        assert!(map.get(&key4).is_none());

        map.clone_from(&saved_map);
        assert_eq!(*map.get(&key).unwrap(), 42);
        assert!(map.get(&key2).is_none());
        assert!(map.get(&key3).is_none());
        assert!(map.get(&key4).is_none());
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);

        map.clear();
        assert!(map.get(&key).is_none());
        assert!(map.get(&key2).is_none());
        assert!(map.get(&key3).is_none());
        assert!(map.get(&key4).is_none());
        assert!(map.is_empty());
    }
}
