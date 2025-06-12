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
use std::fmt::Debug;

use crate::observation::Observation;
use thiserror::Error;

mod closed;
mod consistent;
mod display;
pub mod graph;
mod observation;
pub mod row;

new_key_type! { pub struct SuffixId; }
new_key_type! { pub struct SymbolId; }

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
    /// and the edge 'weights' are the corresponding symbols (or rather their ids) that trigger the transition.
    graph: DiGraph<FnvHashSet<ObservationRowId>, FnvHashSet<SymbolId>>,

    /// A reverse lookup map for states.
    row_to_state: FnvHashMap<ObservationRowId, FnvHashSet<NodeIndex>>,

    /// The set of transitions from a prefix represented by an observation row
    /// to an extension of that prefix by a given a symbol.
    transitions: FnvHashMap<(ObservationRowId, SymbolId), ObservationRowId>,

    symbols: SecondaryMap<SymbolId, ()>,

    /// All suffixes that we have seen so far. A suffix is a word (a list of symbols) applied in a certain state.
    suffixes: SlotMap<SuffixId, Vec<SymbolId>>,
    /// Reverse lookup map for suffixes. This is used to check if a suffix was already added to the learner.
    suffix_lookup: FnvHashMap<Vec<SymbolId>, SuffixId>,

    initial_row: ObservationRowId,
}

impl<T> Default for StateLearner<T>
where
    T: ObservationValue,
{
    fn default() -> Self {
        let mut graph = DiGraph::<FnvHashSet<ObservationRowId>, FnvHashSet<SymbolId>>::default();
        let mut observation_rows = SlotMap::<ObservationRowId, ObservationRow<T>>::default();
        let initial_row = observation_rows.insert(ObservationRow::new_initial());

        let mut row_to_state = FnvHashMap::<ObservationRowId, FnvHashSet<NodeIndex>>::default();
        let initial_state = graph.add_node(FnvHashSet::from_iter([initial_row]));
        row_to_state.insert(initial_row, FnvHashSet::from_iter([initial_state]));

        let mut slf = Self {
            observation_rows,
            graph,
            row_to_state,
            transitions: Default::default(),
            symbols: Default::default(),
            suffixes: Default::default(),
            suffix_lookup: Default::default(),
            initial_row,
        };
        slf.add_missing_candidates();
        slf
    }
}

impl<T> StateLearner<T> {
    pub fn initial_row(&self) -> ObservationRowId {
        self.initial_row
    }

    pub fn get_row_from_prefix(
        &self,
        prefix: impl IntoIterator<Item = SymbolId>,
    ) -> Option<ObservationRowId> {
        let mut prefix = prefix.into_iter().peekable();

        let mut row_id = self.initial_row;

        if prefix.peek().is_none() {
            return Some(self.initial_row);
        }

        loop {
            let prefix_symbol = prefix.next().unwrap();

            if prefix.peek().is_none() {
                break self.transitions.get(&(row_id, prefix_symbol)).copied();
            } else {
                match self.transitions.get(&(row_id, prefix_symbol)) {
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

    pub fn get_suffix_symbols(
        &self,
        suffix_id: SuffixId,
    ) -> impl Iterator<Item = SymbolId> {
        self.suffixes[suffix_id].iter().copied()
    }

    pub fn get_candidate_row_id(
        &self,
        predecessor: ObservationRowId,
        symbol: SymbolId,
    ) -> Option<ObservationRowId> {
        self.transitions.get(&(predecessor, symbol)).copied()
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

    pub fn next_unobserved_suffix(
        &self,
        observation_row: ObservationRowId,
    ) -> Option<SuffixId> {
        self.suffixes
            .keys()
            .filter(move |suffix_id| {
                matches!(self.observation_rows[observation_row].observation(*suffix_id), Observation::Missing)
            })
            .next()
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

    /// Get the prefix required to transition to the provided row
    pub fn get_row_prefix(
        &self,
        mut row_id: ObservationRowId,
    ) -> impl Iterator<Item = SymbolId> {
        let mut prefix = Vec::new();
        while let Some((predecessor, transition_symbol)) = self.observation_rows[row_id].predecessor_transition() {
            prefix.push(transition_symbol);
            row_id = predecessor;
        }

        prefix.into_iter().rev()
    }

    /// Find the next [ObservationRow] that has missing observations.
    /// If `start_row` is provided, the search will start at that row.
    /// Otherwise, the search will start at the first row that has missing observations.
    /// If no row has missing observations, None is returned.
    pub fn next_row_with_missing_observations(
        &self,
        start_row: Option<ObservationRowId>,
    ) -> Option<(ObservationRowId, impl Iterator<Item = SymbolId> + Clone + use<T>)> {
        let start_row = start_row.unwrap_or(self.initial_row);

        let mut frontier = vec![start_row];
        while let Some(current_row) = frontier.pop() {
            // check if current_row has missing observations
            if let Some(unobserved_suffix) = self.unobserved_suffixes(current_row).next() {
                // reconstruct partial prefix and return
                let mut prefix = Vec::new();
                let mut row = current_row;
                while row != start_row
                    && let Some((predecessor_row, transition_symbol)) =
                        self.observation_rows[row].predecessor_transition()
                {
                    prefix.push(transition_symbol);
                    row = predecessor_row;
                }

                return Some((current_row, prefix.into_iter().rev()));
            }

            // process all potential successors
            for symbol_id in self.symbols.keys() {
                match self.transitions.get(&(current_row, symbol_id)).copied() {
                    None => {
                        // end of path
                    }
                    Some(next_row) => {
                        frontier.push(next_row);
                    }
                }
            }
        }

        None
    }
}

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    fn promote_candidate_to_state(
        &mut self,
        candidate_id: ObservationRowId,
    ) {
        let row = &mut self.observation_rows[candidate_id];
        let Some((predecessor_row, transition_symbol)) = row.predecessor_transition() else {
            panic!("Candidate states must have a predecessor state.")
        };

        // move up ^~^
        // make the row great again by making it a state
        row.make_state();

        // Add the new candidates that are now possible due to the new state.
        self.add_missing_candidates();

        self.update_graph_for_new_observation(candidate_id);
    }

    /// Add the provided suffix into the state learner. Return the
    /// new suffix id or the old suffix id if suffix already exists.
    pub fn add_suffix(
        &mut self,
        suffix: impl IntoIterator<Item = SymbolId>,
    ) -> SuffixId {
        let suffix = suffix.into_iter().collect::<Vec<_>>();
        let new_suffix = (0..suffix.len())
            .rev()
            .map(|suffix_end| {
                let suffix = suffix[suffix_end..].to_vec();
                self.add_symbols(suffix.iter().copied());

                match self.suffix_lookup.entry(suffix.clone()) {
                    Entry::Occupied(entry) => {
                        /* The suffix was already added. */
                        *entry.get()
                    }
                    Entry::Vacant(entry) => {
                        let new_suffix = self.suffixes.insert(suffix);
                        entry.insert(new_suffix);
                        new_suffix
                    }
                }
            })
            .last()
            .unwrap();

        new_suffix
    }

    fn add_symbols(
        &mut self,
        symbols: impl IntoIterator<Item = SymbolId>,
    ) {
        for symbol in symbols {
            self.symbols.insert(symbol, ());
        }
        self.add_missing_candidates();
    }

    /// Add the provided suffixes into the state learner.
    /// Any existing suffixes will be skipped.
    pub fn add_suffixes(
        &mut self,
        suffixes: impl IntoIterator<Item = impl IntoIterator<Item = SymbolId>>,
    ) {
        suffixes.into_iter().for_each(|suffix| {
            self.add_suffix(suffix);
        });

        // Add the new candidates that are now possible due to the new suffixes.
        self.add_missing_candidates();
    }

    fn add_missing_candidates(&mut self) {
        let new_candidates = self
            .symbols
            .keys()
            .cartesian_product(
                self.observation_rows
                    .iter()
                    .filter_map(|(row_id, row)| row.is_state().then_some(row_id)),
            )
            .filter_map(|(symbol, row_id)| {
                if !self.transitions.contains_key(&(row_id, symbol)) {
                    // If neither table contains a row for (prefix, symbol) then we need to add a new candidate.
                    Some(ObservationRow::<T>::new_candidate(row_id, symbol))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for new_candidate in new_candidates.into_iter() {
            let new_row_id = self.observation_rows.insert_with_key(|row_id| {
                let transition = new_candidate.predecessor_transition().unwrap();
                self.transitions.insert(transition, row_id);
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
    pub fn add_prefix<I>(
        &mut self,
        prefix: I,
    ) where
        I: IntoIterator<Item = SymbolId>,
        <I as IntoIterator>::IntoIter: Clone,
    {
        let prefix = prefix.into_iter();
        self.add_symbols(prefix.clone());
        let mut prefix = prefix.peekable();
        let mut current_row = self.initial_row;
        // add words and make candidates
        while let Some(symbol) = prefix.next() {
            let row_id = self.transitions[&(current_row, symbol)];
            if self.observation_rows[row_id].is_state() {
                current_row = row_id;
            } else {
                if prefix.peek().is_none() {
                    // If this is the last symbol in the prefix we don't want to promote the candidate.
                    break;
                }

                // We want to promote all intermediate candidates to states.
                self.promote_candidate_to_state(row_id);
                current_row = row_id;
            }
        }
    }
}
