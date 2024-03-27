use std::cell::{
    Ref,
    RefCell,
    RefMut,
};
use std::rc::Rc;

use imbl::HashMap;

use crate::emulator::abstractions::restore::Restorable;
use crate::util::auto_key::AutoKey;
use mogi_system::errors::generic::GenericError;

#[derive(Debug)]
struct Snapshot<T>
where
    T: Restorable,
{
    data: HashMap<AutoKey, Rc<RefCell<T>>>,
}

#[derive(Debug)]
pub struct RestorableHashMap<T>
where
    T: Restorable,
{
    data: HashMap<AutoKey, Rc<RefCell<T>>>,
    next_key: AutoKey,
    snapshots: Vec<Snapshot<T>>,
}

impl<T> Default for RestorableHashMap<T>
where
    T: Restorable,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> RestorableHashMap<T>
where
    T: Restorable,
{
    pub fn new() -> Self {
        Self {
            data: Default::default(),
            next_key: AutoKey(0),
            snapshots: vec![],
        }
    }

    pub fn insert(
        &mut self,
        element: T,
    ) -> AutoKey {
        let key = self.next_key;
        let old = self.data.insert(key, Rc::new(RefCell::new(element)));
        // Due to our key always incrementing, there will never be an old element.
        assert!(old.is_none());
        self.next_key = AutoKey(key.0.checked_add(1).expect("We exhausted 2^64 keys. How?"));
        key
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
    ) -> Option<Ref<'_, T>> {
        self.data.get(key).map(|v| v.borrow())
    }

    pub fn get_mut(
        &mut self,
        key: &AutoKey,
    ) -> Option<RefMut<'_, T>> {
        self.data.get_mut(key).map(|v| v.borrow_mut())
    }

    pub fn remove(
        &mut self,
        key: &AutoKey,
    ) {
        self.data.remove(key);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AutoKey, Ref<'_, T>)> {
        self.data.iter().map(|(k, v)| (k, v.borrow()))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&AutoKey, RefMut<'_, T>)> {
        self.data.iter().map(|(k, v)| (k, v.borrow_mut()))
    }

    pub fn values(&self) -> impl Iterator<Item = Ref<'_, T>> {
        self.data.values().map(|v| v.borrow())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = RefMut<'_, T>> {
        self.data.values().map(|v| v.borrow_mut())
    }
}

impl<T> Restorable for RestorableHashMap<T>
where
    T: Restorable,
{
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.data.iter().for_each(|(k, v)| {
            v.borrow_mut().take_snapshot();
        });
        self.snapshots.push(Snapshot {
            data: self.data.clone(),
        });
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.pop();
        if let Some(snapshot) = self.snapshots.last() {
            self.data = snapshot.data.clone();
            self.data.iter().for_each(|(k, v)| {
                v.borrow_mut()
                    .discard_current_and_restore_previous_snapshot();
            });
        } else {
            self.data = Default::default();
        }
        Ok(())
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        if let Some(snapshot) = self.snapshots.last() {
            self.data = snapshot.data.clone();
            self.data.iter().for_each(|(k, v)| {
                v.borrow_mut().restore_snapshot();
            });
        } else {
            self.data = Default::default();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::ops::AddAssign;

    use super::*;

    #[derive(Debug)]
    struct UInt32 {
        data: u32,
        snaps: Vec<u32>,
    }
    impl PartialEq for UInt32 {
        fn eq(
            &self,
            other: &Self,
        ) -> bool {
            self.data == other.data
        }
    }
    impl AddAssign for UInt32 {
        fn add_assign(
            &mut self,
            rhs: Self,
        ) {
            self.data += rhs.data;
        }
    }

    impl Restorable for UInt32 {
        fn take_snapshot(&mut self) -> Result<(), GenericError> {
            self.snaps.push(self.data);
            Ok(())
        }

        fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
            self.snaps.pop();
            Ok(())
        }

        fn restore_snapshot(&mut self) -> Result<(), GenericError> {
            self.data = *self.snaps.last().unwrap();
            Ok(())
        }
    }

    impl From<u32> for UInt32 {
        fn from(value: u32) -> Self {
            Self {
                data: value,
                snaps: vec![],
            }
        }
    }

    #[test]
    fn restore_correclty_restores_data() {
        let mut map = RestorableHashMap::<UInt32>::new();
        let key = map.insert(42.into());
        let key2 = map.insert(1337.into());
        map.remove(&key2);
        assert_eq!(*map.get(&key).unwrap(), 42.into());
        assert!(map.get(&key2).is_none());

        map.take_snapshot();
        let key3 = map.insert(123.into());
        let key4 = map.insert(1337.into());
        assert_eq!(*map.get(&key).unwrap(), 42.into());
        *map.get_mut(&key).unwrap() += 1.into();
        assert_eq!(*map.get(&key).unwrap(), 43.into());
        assert!(map.get(&key2).is_none());
        assert_eq!(*map.get(&key3).unwrap(), 123.into());
        assert_eq!(*map.get(&key4).unwrap(), 1337.into());

        map.restore_snapshot();
        assert_eq!(*map.get(&key).unwrap(), 42.into());
        assert!(map.get(&key2).is_none());
        assert!(map.get(&key3).is_none());
        assert!(map.get(&key4).is_none());

        map.restore_snapshot();
        assert_eq!(*map.get(&key).unwrap(), 42.into());
        assert!(map.get(&key2).is_none());
        assert!(map.get(&key3).is_none());
        assert!(map.get(&key4).is_none());
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);

        map.discard_current_and_restore_previous_snapshot();
        assert!(map.get(&key).is_none());
        assert!(map.get(&key2).is_none());
        assert!(map.get(&key3).is_none());
        assert!(map.get(&key4).is_none());
        assert!(map.is_empty());
    }
}
