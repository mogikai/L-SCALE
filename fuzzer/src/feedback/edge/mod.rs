use crate::api::error::FuzzerError;
use crate::api::observer::HasObservation;
use crate::api::reducer::Reducer;
use crate::observers::emulator::edge::EdgeObserver;
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

#[derive(Debug)]
struct EdgeFeedback {
    observer: Rc<RefCell<EdgeObserver>>,
    hit_map: Vec<u64>,
}

impl Reducer for EdgeFeedback {
    type Output = bool;

    fn reduce(&mut self) -> Result<Self::Output, FuzzerError> {
        let observer = self.observer.borrow();
        let current_edge_counts = observer.observation();

        self.hit_map.resize(current_edge_counts.len(), 0);
        let mut interesting = false;
        for (current, maximum) in current_edge_counts.iter().zip(self.hit_map.iter_mut()) {
            if current > maximum {
                *maximum = *current;
                interesting = true;
            }
        }
        Ok(interesting)
    }
}
