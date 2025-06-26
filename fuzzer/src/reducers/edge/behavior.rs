use crate::api::error::FuzzerError;
use crate::api::reducer::Reducer;
use crate::core::luzzer::Observation;
use crate::observers::bb_based::edge::EdgeHitObserver;
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

/// This reducer reduces an edge observation to a testcase behavior. In the future it might be necessary
/// to split this up into a `TestCaseEdgeBehavior` that is then consumed by a `TestCaseBehaviorReducer`
/// that consumes multiple different TestCaseXBehaviors
#[derive(Debug, Default)]
pub struct EdgeBehaviorReducer {
    #[allow(unused)]
    observer: Rc<RefCell<EdgeHitObserver>>,
}

impl EdgeBehaviorReducer {
    pub fn new(observer: Rc<RefCell<EdgeHitObserver>>) -> Self {
        Self { observer }
    }
}

impl Reducer for EdgeBehaviorReducer {
    type Output = Observation;

    fn reduce(&mut self) -> Result<Self::Output, FuzzerError> {
        // let observer = self.observer.borrow();
        // let current_edge_hits = observer.observation();
        // debug!(
        //     "Edge hits: {:?}",
        //     current_edge_hits
        //         .iter()
        //         .flat_map(|v| if *v { "1".chars() } else { "0".chars() })
        //         .collect::<String>()
        // );
        // let mut hasher = DefaultHasher::new();
        // current_edge_hits.hash(&mut hasher);
        // Ok(Observation::new(hasher.finish()))
        todo!()
    }
}
