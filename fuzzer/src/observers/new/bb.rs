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
    sparse: bool,
}

impl BasicBlockObserver {
    pub fn new(
        start: EVAddr,
        end: EVAddr,
        sparse: bool,
    ) -> Self {
        Self {
            start,
            hitcounts: (0..end - start).map(|i| (i % 256) as u8).collect(),
            sparse,
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
    type RawObservation = (EVAddr, usize);

    fn observe(
        &mut self,
        observation: &Self::RawObservation,
    ) {
        // If sparse is configured we only mark the first address of the executed basic block.
        let bb_size = if self.sparse { 1 } else { observation.1 };
        for offset in 0..bb_size {
            let entry = unsafe {
                self.hitcounts
                    .get_unchecked_mut(observation.0 - self.start + offset)
            };
            *entry = entry.wrapping_add(1);
        }
    }
}

impl HasObservation for BasicBlockObserver {
    type Observation = [u8];

    fn observation(&self) -> impl Deref<Target = Self::Observation> {
        self.hitcounts.as_slice()
    }
}
