mod metadata;

use std::fmt::Debug;

use crate::feedback::edge::metadata::EdgeFeedbackMetadata;
use crate::observers::edge::EdgeObserver;
use libafl::events::EventFirer;
use libafl::executors::ExitKind;
use libafl::feedbacks::Feedback;
use libafl::inputs::UsesInput;
use libafl::observers::ObserversTuple;
use libafl::state::{
    HasClientPerfMonitor,
    HasNamedMetadata,
};
use libafl_bolts::{
    Error,
    Named,
};

#[derive(Debug)]
pub struct EdgeFeedback {
    observer_name: String,
}

impl EdgeFeedback {
    pub fn new(edge_observer: &EdgeObserver) -> Self {
        Self {
            observer_name: edge_observer.name().to_string(),
        }
    }
}

impl Named for EdgeFeedback {
    fn name(&self) -> &str {
        "EdgeFeedback"
    }
}

impl<StateT> Feedback<StateT> for EdgeFeedback
where
    StateT: UsesInput + HasClientPerfMonitor + HasNamedMetadata,
{
    fn init_state(
        &mut self,
        state: &mut StateT,
    ) -> Result<(), Error> {
        // Initialize `MapFeedbackMetadata` with an empty vector and add it to the state.
        // The `MapFeedbackMetadata` would be resized on-demand in `is_interesting`
        state.add_named_metadata(EdgeFeedbackMetadata::default(), self.name());
        Ok(())
    }
    fn is_interesting<EventManagerT, ObserversTupleT>(
        &mut self,
        state: &mut StateT,
        manager: &mut EventManagerT,
        input: &StateT::Input,
        observers: &ObserversTupleT,
        exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EventManagerT: EventFirer<State = StateT>,
        ObserversTupleT: ObserversTuple<StateT>,
    {
        let mut interesting = false;
        // TODO Replace with match_name_type when stable
        let observer = observers
            .match_name::<EdgeObserver>(&self.observer_name)
            .unwrap();

        let observer_state = state
            .named_metadata_map_mut()
            .get_mut::<EdgeFeedbackMetadata>(self.name())
            .unwrap();
        // let size = observer.usable_count();
        // let len = observer.len();
        // if observer_state.history_map.len() < len {
        //     observer_state.history_map.resize(len, u8::default());
        // }
        //
        // let map = observer.as_slice();
        // debug_assert!(map.len() >= size);
        //
        // let history_map = observer_state.history_map.as_slice();

        // Non vector implementation for reference
        /*for (i, history) in history_map.iter_mut().enumerate() {
            let item = map[i];
            let reduced = MaxReducer::reduce(*history, item);
            if DifferentIsNovel::is_novel(*history, reduced) {
                *history = reduced;
                interesting = true;
                if self.novelties.is_some() {
                    self.novelties.as_mut().unwrap().push(i);
                }
            }
        }*/
        //
        // let steps = size / VectorType::LANES;
        // let left = size % VectorType::LANES;
        //
        // if let Some(novelties) = self.novelties.as_mut() {
        //     novelties.clear();
        //     for step in 0..steps {
        //         let i = step * VectorType::LANES;
        //         let history = VectorType::from_slice(&history_map[i..]);
        //         let items = VectorType::from_slice(&map[i..]);
        //
        //         if items.simd_max(history) != history {
        //             interesting = true;
        //             unsafe {
        //                 for j in i..(i + VectorType::LANES) {
        //                     let item = *map.get_unchecked(j);
        //                     if item > *history_map.get_unchecked(j) {
        //                         novelties.push(j);
        //                     }
        //                 }
        //             }
        //         }
        //     }
        //
        //     for j in (size - left)..size {
        //         unsafe {
        //             let item = *map.get_unchecked(j);
        //             if item > *history_map.get_unchecked(j) {
        //                 interesting = true;
        //                 novelties.push(j);
        //             }
        //         }
        //     }
        // } else {
        //     for step in 0..steps {
        //         let i = step * VectorType::LANES;
        //         let history = VectorType::from_slice(&history_map[i..]);
        //         let items = VectorType::from_slice(&map[i..]);
        //
        //         if items.simd_max(history) != history {
        //             interesting = true;
        //             break;
        //         }
        //     }
        //
        //     if !interesting {
        //         for j in (size - left)..size {
        //             unsafe {
        //                 let item = *map.get_unchecked(j);
        //                 if item > *history_map.get_unchecked(j) {
        //                     interesting = true;
        //                     break;
        //                 }
        //             }
        //         }
        //     }
        // }
        //
        // let initial = observer.initial();
        // if interesting {
        //     let len = history_map.len();
        //     let filled = history_map.iter().filter(|&&i| i != initial).count();
        //     // opt: if not tracking optimisations, we technically don't show the *current* history
        //     // map but the *last* history map; this is better than walking over and allocating
        //     // unnecessarily
        //     manager.fire(
        //         state,
        //         Event::UpdateUserStats {
        //             name: self.stats_name.to_string(),
        //             value: UserStats::Ratio(
        //                 self.novelties
        //                     .as_ref()
        //                     .map_or(filled, |novelties| filled + novelties.len()) as u64,
        //                 len as u64,
        //             ),
        //             phantom: PhantomData,
        //         },
        //     )?;
        // }

        Ok(interesting)
    }
}
