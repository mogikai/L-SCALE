use std::fmt::Debug;
use std::ops::Deref;

pub trait Observer {
    /// Makes the observer gather data if it has not already done so during the execution of the target.
    fn gather(&mut self);

    /// Clears the observation data such that the next run can start from a clean slate.
    fn clear(&mut self);
}

pub trait Observe: Send + Sync {
    /// The type of observation this Observer makes. This differs from the type of observation it makes
    /// available to consumers via the HasObservation trait. In most cases the observations will have been
    /// transformed in some way before making them available in HasObservation.
    type RawObservation: ?Sized;

    /// This function is usually called by meters in the emulator, after the observer has been registered
    /// in a meter.
    fn observe(
        &mut self,
        observation: &Self::RawObservation,
    );
}

/// This trait signals that the object in question has an observation that it makes available.
pub trait HasObservation: Observer {
    type Observation: ?Sized;

    fn observation(&self) -> impl Deref<Target = Self::Observation>;
}

pub trait DebuggableObserver: Observer + Debug {}

impl<T> DebuggableObserver for T where T: Observer + Debug {}

pub trait DebuggableObserve: Observe + Debug {}
impl<T> DebuggableObserve for T where T: Observe + Debug {}
