use std::ops::Deref;

pub trait Observer {
    /// Makes the observer gather data if it has not already done so during the execution of the target.
    fn gather(&mut self);

    /// Clears the observation data such that the next run can start from a clean slate.
    fn clear(&mut self);
}

pub trait HasObservation: Observer {
    type Observation: ?Sized;

    fn observation(&self) -> impl Deref<Target = Self::Observation>;
}
