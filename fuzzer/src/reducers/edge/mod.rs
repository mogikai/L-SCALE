pub mod behavior;

use crate::api::error::FuzzerError;
use crate::api::observer::HasObservation;
use crate::api::reducer::Reducer;
use crate::observers::bb_based::edge::EdgeHitCountObserver;
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

/// This reducer keeps track of the edges that have been observed until now.
/// If it finds a new edge that was not observed previously, the reducer will
/// output true. If the edge has been observed already, it will output false.
#[derive(Debug, Default)]
pub struct InterestingEdgesReducer {
    observer: Rc<RefCell<EdgeHitCountObserver>>,
    // TODO: Make this (and all reducer state) target state specific. I.e. we need to refactor this into the
    //       global fuzzer state and pass it in from the outside. Alternatively we can make this stateless
    //       by passing in the historical map from the outside via a HistoryObserver. That way the reducers
    //       would always be stateless, and only observers would need to have state.
    hit_map: Vec<u64>,
}

impl InterestingEdgesReducer {
    pub fn new(observer: Rc<RefCell<EdgeHitCountObserver>>) -> Self {
        Self {
            observer,
            ..Default::default()
        }
    }
}

impl Reducer for InterestingEdgesReducer {
    type Output = bool;

    fn reduce(&mut self) -> Result<Self::Output, FuzzerError> {
        let observer = self.observer.borrow();
        let current_edge_counts = observer.observation();

        if self.hit_map.len() < current_edge_counts.len() {
            self.hit_map.resize(current_edge_counts.len(), 0);
        }
        let mut interesting = false;
        for (current, maximum) in current_edge_counts.iter().zip(self.hit_map.iter_mut()) {
            if *current > (*maximum << 1) {
                *maximum = *current;
                interesting = true;
            }
        }
        Ok(interesting)
    }
}
