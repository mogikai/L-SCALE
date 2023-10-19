use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// A wrapper for a cloneable data type `T` that performs lazy cloning only on write access to any of the
/// cloned pointers.
#[derive(Clone, Debug, Default)]
pub struct RCow<T>(Rc<T>)
where
    T: Clone;

impl<T> RCow<T>
where
    T: Clone,
{
    pub fn new(item: T) -> Self {
        Self(Rc::new(item))
    }
}

impl<T> Deref for RCow<T>
where
    T: Clone,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T> DerefMut for RCow<T>
where
    T: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        Rc::make_mut(&mut self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut x = RCow::new(5);
        *x += 5;
        assert_eq!(*x, 10);

        let mut y = x.clone();
        assert_eq!(*x, 10);
        assert_eq!(*y, 10);

        *y -= 10;
        assert_eq!(*x, 10);
        assert_eq!(*y, 0);
    }
}
