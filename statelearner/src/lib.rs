use crate::graph::StateLearnerError;
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
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use std::collections::hash_map::Entry;

mod closed;
mod consistent;
pub mod graph;
mod ovservation;
mod row;

new_key_type! { pub struct SuffixId; }
new_key_type! { pub struct WordId; }

pub trait ObservationValue: PartialEq + Clone {}

pub type StateLearnerResult<T = ()> = Result<T, StateLearnerError>;

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

impl<T> StateLearner<T> {}

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

        self.update_graph_for_new_observation(candidate_id);

        // Add the new candidates that are now possible due to the new state.
        self.add_missing_candidates();
    }

    /// Add the provided suffix into the state learner. Return the
    /// new suffix id or the old suffix id if suffix already exists.
    pub fn add_suffix(
        &mut self,
        suffix: impl IntoIterator<Item = WordId>,
    ) -> SuffixId {
        let suffix = suffix.into_iter().collect::<Vec<_>>();

        let new_suffix = match self.suffix_lookup.entry(suffix.clone()) {
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
        };

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
            let suffix = suffix.into_iter().collect::<Vec<_>>();
            match self.suffix_lookup.entry(suffix.clone()) {
                Entry::Occupied(_) => { /* The suffix was already added. */ }
                Entry::Vacant(entry) => {
                    for word in suffix.iter() {
                        self.words.insert(*word, ());
                    }
                    entry.insert(self.suffixes.insert(suffix));
                }
            }
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
}
