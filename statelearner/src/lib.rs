use crate::row::{
    ObservationRow,
    ObservationRowId,
};
use fnv::{
    FnvHashMap,
    FnvHashSet,
};
use itertools::Itertools;
use petgraph::graph::{
    DiGraph,
    NodeIndex,
};
use slotmap::{
    SecondaryMap,
    SlotMap,
    new_key_type,
};
use std::collections::hash_map::Entry;
use std::fmt::{
    Debug,
    Display,
};

use crate::ovservation::Observation;
use std::fmt::Write;
use thiserror::Error;

mod closed;
mod consistent;
mod display;
pub mod graph;
mod ovservation;
mod row;

new_key_type! { pub struct SuffixId; }
new_key_type! { pub struct WordId; }

pub trait ObservationValue: PartialEq + Clone {}

pub type StateLearnerResult<T = ()> = Result<T, StateLearnerError>;

#[derive(Error, Debug)]
pub enum StateLearnerError {
    #[error("Conflicting observations. Setting an observation to a different value for a suffix is not allowed.")]
    ConflictingObservation,
    #[error("Unknown error")]
    Unknown,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct StateLearner<T> {
    /// Contains a row of observations for each state or candidate state.
    observation_rows: SlotMap<ObservationRowId, ObservationRow<T>>,
    /// The graph that contains the states and the edges between them.
    /// The node 'weights' are the corresponding observation rows (or rather their ids)
    /// and the edge 'weights' are the corresponding words (or rather their ids) that trigger the transition.
    graph: DiGraph<FnvHashSet<ObservationRowId>, FnvHashSet<WordId>>,

    /// A reverse lookup map for states.
    row_to_state: FnvHashMap<ObservationRowId, FnvHashSet<NodeIndex>>,

    state_transitions: FnvHashMap<(ObservationRowId, WordId), ObservationRowId>,

    /// A set of candidate states. A candidate state is a state that can be arrived at
    /// by applying a word to an existing state. The number of candidates is thus at most
    /// |V| * |W| where |V| is the number of states and |W| is the number of words.
    /// A candidate state is identified by its corresponding observation row.
    candidate_transitions: FnvHashMap<(ObservationRowId, WordId), ObservationRowId>,

    words: SecondaryMap<WordId, ()>,

    /// All suffixes that we have seen so far. A suffix is a list of words applied in a certain state.
    suffixes: SlotMap<SuffixId, Vec<WordId>>,
    /// Reverse lookup map for suffixes. This is used to check if a suffix was already added to the learner.
    suffix_lookup: FnvHashMap<Vec<WordId>, SuffixId>,

    initial_row: ObservationRowId,
}

impl<T> StateLearner<T> {
    pub fn initial_row(&self) -> ObservationRowId {
        self.initial_row
    }

    pub fn get_state_row_id_from_prefix(
        &self,
        prefix: impl IntoIterator<Item = WordId>,
    ) -> Option<ObservationRowId> {
        let mut row_id = self.initial_row;
        for prefix_word in prefix {
            match self.state_transitions.get(&(row_id, prefix_word)) {
                Some(next_row_id) => {
                    row_id = *next_row_id;
                }
                None => {
                    return None;
                }
            }
        }

        Some(row_id)
    }

    pub fn get_candidate_row_id_from_prefix(
        &self,
        prefix: impl IntoIterator<Item = WordId>,
    ) -> Option<ObservationRowId> {
        let mut prefix = prefix.into_iter().peekable();

        let mut row_id = self.initial_row;
        loop {
            let prefix_word = prefix.next().unwrap();

            if prefix.peek().is_none() {
                break self
                    .candidate_transitions
                    .get(&(row_id, prefix_word))
                    .copied();
            } else {
                match self.state_transitions.get(&(row_id, prefix_word)) {
                    Some(next_row_id) => {
                        row_id = *next_row_id;
                    }
                    None => {
                        break None;
                    }
                }
            }
        }
    }

    pub fn get_suffix_words(
        &self,
        suffix_id: SuffixId,
    ) -> impl Iterator<Item = WordId> {
        self.suffixes[suffix_id].iter().copied()
    }

    pub fn get_candidate_row_id(
        &self,
        predecessor: ObservationRowId,
        word: WordId,
    ) -> Option<ObservationRowId> {
        self.candidate_transitions
            .get(&(predecessor, word))
            .copied()
    }

    /// Returns an iterator over all suffixes that have missing observations
    pub fn unobserved_suffixes(
        &self,
        observation_row: ObservationRowId,
    ) -> impl Iterator<Item = SuffixId> {
        self.suffixes.keys().filter(move |suffix_id| {
            matches!(self.observation_rows[observation_row].observation(*suffix_id), Observation::Missing)
        })
    }

    pub fn state_observation_rows(
        &self,
        state: NodeIndex,
    ) -> impl Iterator<Item = ObservationRowId> {
        self.graph
            .node_weight(state)
            .into_iter()
            .map(|set| set.iter())
            .flatten()
            .copied()
    }

    pub fn rows_with_missing_observations(&self) -> impl Iterator<Item = ObservationRowId> {
        self.observation_rows
            .keys()
            .filter(|row| self.unobserved_suffixes(*row).next().is_some())
    }

    pub fn next_row_with_missing_observations(&self) -> Option<ObservationRowId> {
        self.observation_rows
            .keys()
            .find(|row| self.unobserved_suffixes(*row).next().is_some())
    }
}

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    pub fn new() -> Self {
        let mut graph = DiGraph::<FnvHashSet<ObservationRowId>, FnvHashSet<WordId>>::default();
        let mut observation_rows = SlotMap::<ObservationRowId, ObservationRow<T>>::default();
        let initial_row = observation_rows.insert(ObservationRow::new_initial());

        let mut row_to_state = FnvHashMap::<ObservationRowId, FnvHashSet<NodeIndex>>::default();
        let initial_state = graph.add_node(FnvHashSet::from_iter([initial_row]));
        row_to_state.insert(initial_row, FnvHashSet::from_iter([initial_state]));

        let mut slf = Self {
            observation_rows,
            graph,
            row_to_state,
            state_transitions: Default::default(),
            candidate_transitions: Default::default(),
            words: Default::default(),
            suffixes: Default::default(),
            suffix_lookup: Default::default(),
            initial_row,
        };
        slf.add_missing_candidates();
        slf
    }

    fn promote_candidate_to_state(
        &mut self,
        candidate_id: ObservationRowId,
    ) {
        let row = &mut self.observation_rows[candidate_id];
        let Some((predecessor_row, transition_word)) = row.predecessor_transition() else {
            panic!("Candidate states must have a predecessor state.")
        };

        // move up ^~^
        // Move the candidate from the candidate_transitions table into the
        // state_transitions table
        self.state_transitions
            .insert((predecessor_row, transition_word), candidate_id);
        self.candidate_transitions
            .remove(&(predecessor_row, transition_word));
        row.make_state();

        // Add the new candidates that are now possible due to the new state.
        self.add_missing_candidates();

        self.update_graph_for_new_observation(candidate_id);
    }

    /// Add the provided suffix into the state learner. Return the
    /// new suffix id or the old suffix id if suffix already exists.
    pub fn add_suffix(
        &mut self,
        suffix: impl IntoIterator<Item = WordId>,
    ) -> SuffixId {
        let suffix = suffix.into_iter().collect::<Vec<_>>();
        let new_suffix = (0..suffix.len())
            .rev()
            .map(|suffix_end| {
                let suffix = suffix[suffix_end..].to_vec();

                match self.suffix_lookup.entry(suffix.clone()) {
                    Entry::Occupied(entry) => {
                        /* The suffix was already added. */
                        *entry.get()
                    }
                    Entry::Vacant(entry) => {
                        for word in suffix.iter() {
                            self.words.insert(*word, ());
                        }
                        let new_suffix = self.suffixes.insert(suffix);
                        entry.insert(new_suffix);
                        new_suffix
                    }
                }
            })
            .last()
            .unwrap();

        // Add the new candidates that are now possible due to the new suffixes.
        self.add_missing_candidates();

        new_suffix
    }

    /// Add the provided suffixes into the state learner.
    /// Any existing suffixes will be skipped.
    pub fn add_suffixes(
        &mut self,
        suffixes: impl IntoIterator<Item = impl IntoIterator<Item = WordId>>,
    ) {
        suffixes.into_iter().for_each(|suffix| {
            self.add_suffix(suffix);
        });

        // Add the new candidates that are now possible due to the new suffixes.
        self.add_missing_candidates();
    }

    pub fn get_row_prefix(
        &self,
        mut row_id: ObservationRowId,
    ) -> impl Iterator<Item = WordId> {
        let mut words = Vec::new();
        while let Some((predecessor, transition_word)) = self.observation_rows[row_id].predecessor_transition() {
            words.push(transition_word);
            row_id = predecessor;
        }

        words.into_iter().rev()
    }

    fn add_missing_candidates(&mut self) {
        let new_candidates = self
            .words
            .keys()
            .cartesian_product(
                self.observation_rows
                    .iter()
                    .filter_map(|(row_id, row)| row.is_state().then_some(row_id)),
            )
            .filter_map(|(word, row_id)| {
                if !self.candidate_transitions.contains_key(&(row_id, word))
                    && !self.state_transitions.contains_key(&(row_id, word))
                {
                    // If neither table contains a row for (prefix, word) then we need to add a new candidate.
                    Some(ObservationRow::<T>::new_candidate(row_id, word))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for new_candidate in new_candidates.into_iter() {
            let new_row_id = self.observation_rows.insert_with_key(|row_id| {
                let transition = new_candidate.predecessor_transition().unwrap();
                self.candidate_transitions.insert(transition, row_id);
                new_candidate
            });
            self.update_graph_for_new_observation(new_row_id);
        }
    }

    pub fn add_observation(
        &mut self,
        observation_row: ObservationRowId,
        suffix_id: SuffixId,
        observation: T,
    ) -> StateLearnerResult {
        self.observation_rows[observation_row].set_observation(suffix_id, observation)?;

        if self.row_to_state.contains_key(&observation_row) {
            self.update_graph_for_new_observation(observation_row);
        }

        Ok(())
    }

    /// Insert the given prefix into the state learner.
    /// This potentially adds multiple new states and
    /// state candidates to the observation table
    pub fn add_prefix(
        &mut self,
        prefix: impl IntoIterator<Item = WordId>,
    ) {
        let mut prefix = prefix.into_iter().peekable();
        let mut current_row = self.initial_row;
        while let Some(word) = prefix.next() {
            if let Some(next_row) = self.state_transitions.get(&(current_row, word)) {
                current_row = *next_row;
            } else {
                if prefix.peek().is_none() {
                    // If this is the last word in the prefix we don't want to promote the candidate.
                    break;
                }

                // We want to promote all intermediate candidates to states.
                let candidate = self.candidate_transitions[&(current_row, word)];
                self.promote_candidate_to_state(candidate);
                current_row = candidate;
            }
        }
    }
}
