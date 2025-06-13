use crate::api::observer::{
    HasObservation,
    Observe,
    Observer,
};
use mogi_utilities::addresses::EVAddr;
use std::ops::Deref;

#[derive(Debug)]
pub struct BasicBlockObserver {
    start: EVAddr,
    hitcounts: Vec<u8>,
}

impl BasicBlockObserver {
    pub fn new(
        start: EVAddr,
        end: EVAddr,
    ) -> Self {
        Self {
            start,
            hitcounts: (0..end - start).map(|i| (i % 256) as u8).collect(),
        }
    }
}

impl Observer for BasicBlockObserver {
    fn gather(&mut self) {}

    fn clear(&mut self) {
        for i in 0..self.hitcounts.len() {
            self.hitcounts[i] = (i % 256) as u8;
        }
    }
}

impl Observe for BasicBlockObserver {
    type RawObservation = EVAddr;

    fn observe(
        &mut self,
        observation: &Self::RawObservation,
    ) {
        let entry = unsafe { self.hitcounts.get_unchecked_mut(*observation - self.start) };
        *entry = entry.wrapping_add(1);
    }
}

impl HasObservation for BasicBlockObserver {
    type Observation = [u8];

    fn observation(&self) -> impl Deref<Target = Self::Observation> {
        self.hitcounts.as_slice()
    }
}
