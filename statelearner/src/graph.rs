use itertools::Itertools;
use petgraph::dot::Dot;
use petgraph::graph::{
    DiGraph,
    NodeIndex,
};
use petgraph::visit::{
    IntoNodeIdentifiers,
    NodeRef,
};
use slotmap::{
    Key,
    SecondaryMap,
    SlotMap,
    new_key_type,
};
use std::collections::hash_map::Entry;
use std::collections::{
    BTreeSet,
    HashMap,
};
use std::fmt::{
    Debug,
    Display,
    Write,
};
use std::fs::File;
use std::iter::once;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateLearnerError {
    #[error("Conflicting observations. Setting an observation to a different value for a suffix is not allowed.")]
    ConflictingObservation,
    #[error("Unknown error")]
    Unknown,
}

pub type StateLearnerResult<T = ()> = Result<T, StateLearnerError>;

pub trait ObservationValue: PartialEq + Clone {}

#[derive(Default, Debug, Clone)]
enum Observation<T> {
    #[default]
    Missing,
    Observed(T),
}

enum ObservationComparison {
    Equal,
    NotEqual,
    Undetermined,
}

impl<T> Observation<T>
where
    T: ObservationValue,
{
    fn unwrap(&self) -> &T {
        match self {
            Observation::Missing => {
                panic!("unwrap on missing observation")
            }
            Observation::Observed(observation) => observation,
        }
    }

    fn compare(
        &self,
        other: &Self,
    ) -> ObservationComparison {
        // If all observations are equal, then the comparison returns equal.
        // If one or more differences are found, then the comparison is not equal.
        // If one or both observations are missing, then the comparison is undetermined.
        match (self, other) {
            (Observation::Observed(a), Observation::Observed(b)) if a == b => ObservationComparison::Equal,
            (Observation::Observed(a), Observation::Observed(b)) if a != b => ObservationComparison::NotEqual,
            _ => ObservationComparison::Undetermined,
        }
    }
}

impl ObservationValue for String {}

new_key_type! { pub struct ObservationRowId; }
new_key_type! { pub struct SuffixId; }
new_key_type! { pub struct WordId; }

impl Display for ObservationRowId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Row({:?})", self.data())
    }
}

impl Display for SuffixId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Suffix({:?})", self.data())
    }
}

impl Display for WordId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Word({:?})", self.data())
    }
}

/// A row of observations for a state or candidate state.
/// By default, values in columns that have not been filled are defined as [Observation::Missing].
#[derive(Debug)]
struct ObservationRow<T> {
    /// The observation row that is the predecessor of this row.
    /// This new row is arrived at by applying the transition word to the predecessor row.
    predecessor_row: Option<ObservationRowId>,
    /// The word that is applied to the predecessor row to arrive at this row.
    transition_word: WordId,
    /// The observations when applying the corresponding suffix to the
    /// target in the state represented by this row.
    observations: SecondaryMap<SuffixId, Observation<T>>,
}

impl<T> ObservationRow<T> {
    pub fn new_candidate(
        predecessor: ObservationRowId,
        word: WordId,
    ) -> Self {
        Self {
            predecessor_row: Some(predecessor),
            transition_word: word,
            observations: Default::default(),
        }
    }

    pub fn get_observation(
        &self,
        suffix: SuffixId,
    ) -> &Observation<T> {
        self.observations
            .get(suffix)
            .unwrap_or(&Observation::Missing)
    }
}

impl<T> ObservationRow<T>
where
    T: ObservationValue,
{
    pub fn compare_observations(
        &self,
        other: &Self,
    ) -> ObservationComparison {
        let result = if self.observations.len() != other.observations.len() {
            // If the lengths are not equal, then the comparison is undetermined, since one observation
            // row is missing observations.
            // We cannot shortcut here, since it might still be possible that the rows can be determined
            // to not be equal.
            ObservationComparison::Undetermined
        } else {
            // Otherwise we set the result to equal for now.
            ObservationComparison::Equal
        };

        for (suffix, observation) in self.observations.iter() {
            let other_observation = other
                .observations
                .get(suffix)
                .unwrap_or(&Observation::Missing);
            if let ObservationComparison::NotEqual = observation.compare(other_observation) {
                // If one of the observations is not equal, then the comparison is not equal.
                // Even if some observations are still missing, we can already say for certain
                // that the rows will never be equal.
                return ObservationComparison::NotEqual;
            }
        }

        // If we did not determine the rows to be not equal, then the result must be undetermined or equal.
        result
    }
}

impl<T> ObservationRow<T>
where
    T: Debug,
{
    fn fmt_debug_row(
        &self,
        self_name: &str,
        f: &mut impl Write,
        suffixes: &SlotMap<SuffixId, Vec<WordId>>,
    ) -> std::fmt::Result {
        let pred_str = self
            .predecessor_row
            .map(|pred| format!("{}", pred))
            .unwrap_or("n/a".into());
        write!(f, "{:<40}", format!("({:<10} --[{:^12}]--> {:>10}) ", pred_str, self.transition_word, self_name))?;
        write!(f, "")?;
        for suffix_id in suffixes.keys() {
            if let Some(observation) = self.observations.get(suffix_id)
                && let Observation::Observed(observation) = observation
            {
                write!(f, "{:^20} ", format!("{:?}", observation))?;
            } else {
                write!(f, "{:^20} ", "n/a")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ConsistencyViolation {
    rows: (ObservationRowId, ObservationRowId),
    word_id: WordId,
    suffix_id: SuffixId,
}

impl ConsistencyViolation {
    fn new(
        rows: (ObservationRowId, ObservationRowId),
        word_id: WordId,
        suffix_id: SuffixId,
    ) -> Self {
        Self {
            rows,
            word_id,
            suffix_id,
        }
    }
}

pub struct StateLearner<T> {
    /// Contains a row of observations for each state or candidate state.
    observation_rows: SlotMap<ObservationRowId, ObservationRow<T>>,
    /// The graph that contains the states and the edges between them.
    /// The node 'weights' are the corresponding observation rows (or rather their ids)
    /// and the edge 'weights' are the corresponding words (or rather their ids) that trigger the transition.
    graph: DiGraph<BTreeSet<ObservationRowId>, WordId>,

    /// A reverse lookup map for states.
    row_to_state: HashMap<ObservationRowId, BTreeSet<NodeIndex>>,

    state_transitions: HashMap<(ObservationRowId, WordId), ObservationRowId>,

    /// A set of candidate states. A candidate state is a state that can be arrived at
    /// by applying a word to an existing state. The number of candidates is thus at most
    /// |V| * |W| where |V| is the number of states and |W| is the number of words.
    /// A candidate state is identified by its corresponding observation row.
    candidate_transitions: HashMap<(ObservationRowId, WordId), ObservationRowId>,

    words: SecondaryMap<WordId, ()>,

    /// All suffixes that we have seen so far. A suffix is a list of words applied in a certain state.
    suffixes: SlotMap<SuffixId, Vec<WordId>>,
    /// Reverse lookup map for suffixes. This is used to check if a suffix was already added to the learner.
    suffix_lookup: HashMap<Vec<WordId>, SuffixId>,

    initial_row: ObservationRowId,

    /// The id of the empty word, i.e., a word that consists of zero characters.
    /// This can be considered as simply running the target without applying any input.
    epsilon: WordId,
}

impl<T> StateLearner<T> {
    pub fn new(epsilon: WordId) -> Self {
        let mut graph = DiGraph::<BTreeSet<ObservationRowId>, WordId>::default();
        let mut observation_rows = SlotMap::<ObservationRowId, ObservationRow<T>>::default();
        let initial_row = observation_rows.insert(ObservationRow {
            predecessor_row: None,
            transition_word: epsilon,
            observations: Default::default(),
        });

        let mut row_to_state = HashMap::<ObservationRowId, BTreeSet<NodeIndex>>::default();
        let initial_state = graph.add_node(BTreeSet::from([initial_row]));
        row_to_state.insert(initial_row, BTreeSet::from([initial_state]));

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
            epsilon,
        };
        slf.add_suffixes(once([epsilon]));
        slf.add_missing_candidates();
        slf
    }

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
                // case 1
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

    pub fn get_candidate_row_id(
        &self,
        predecessor: ObservationRowId,
        word: WordId,
    ) -> Option<ObservationRowId> {
        self.candidate_transitions
            .get(&(predecessor, word))
            .copied()
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

    /// Returns an iterator over all suffixes that have missing observations
    pub fn unobserved_suffixes(
        &self,
        observation_row: ObservationRowId,
    ) -> impl Iterator<Item = SuffixId> {
        self.suffixes.keys().filter(move |suffix_id| {
            matches!(
                self.observation_rows[observation_row]
                    .observations
                    .get(*suffix_id)
                    .unwrap_or(&Observation::Missing),
                Observation::Missing
            )
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

    pub fn states_with_missing_observations(&self) -> impl Iterator<Item = ObservationRowId> {
        self.rows_with_missing_observations()
            .filter(|row_id| self.row_to_state.contains_key(&row_id))
    }

    pub fn candidates_with_missing_observations(&self) -> impl Iterator<Item = ObservationRowId> {
        self.rows_with_missing_observations()
            .filter(|row_id| !self.row_to_state.contains_key(&row_id))
    }

    fn add_missing_candidates(&mut self) {
        let new_candidates = self
            .observation_rows
            .keys()
            .filter(|row_id| self.row_to_state.contains_key(row_id))
            .cartesian_product(self.words.keys())
            .filter_map(|candidate| {
                if self.candidate_transitions.contains_key(&candidate)
                    || self.state_transitions.contains_key(&candidate)
                {
                    None
                } else {
                    Some(ObservationRow::<T>::new_candidate(candidate.0, candidate.1))
                }
            })
            .collect::<Vec<_>>();

        for new_candidate in new_candidates.into_iter() {
            self.observation_rows.insert_with_key(|row_id| {
                let pred = new_candidate.predecessor_row.unwrap();
                let word = new_candidate.transition_word;
                self.candidate_transitions.insert((pred, word), row_id);

                new_candidate
            });
        }
    }

    pub fn export(
        &self,
        file: &Path,
    ) {
        let dot = Dot::with_config(&self.graph, &[]);
        let mut file = File::options()
            .truncate(true)
            .write(true)
            .create(true)
            .open(file)
            .unwrap();
        use std::io::Write;
        file.write_fmt(format_args!("{:?}", dot)).unwrap();
    }
}

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    pub fn add_observation(
        &mut self,
        observation_row: ObservationRowId,
        suffix_id: SuffixId,
        observation: T,
    ) -> StateLearnerResult {
        let observations = &mut self.observation_rows[observation_row].observations;
        if let Some(old_observation) = observations.get(suffix_id) {
            if let Observation::Observed(old_observation) = old_observation {
                if old_observation != &observation {
                    Err(StateLearnerError::ConflictingObservation)?
                }
            }
        }
        observations.insert(suffix_id, Observation::Observed(observation));

        if self.row_to_state.contains_key(&observation_row) {
            self.update_graph_for_new_observation(observation_row);
        }

        // TODO: We need to update the graph here if the observation lead to rows now being distinct.
        //       If two rows were equal and are now distinct, this means that we discovered a new
        //       state.
        Ok(())
    }

    pub fn make_closed(&mut self) {
        while let Some(candidate_id) = self.find_closed_property_violation() {
            self.promote_candidate_to_state(candidate_id);
        }
    }

    /// The consistent property is violated if two equal rows exhibit different behavior under a
    /// certain (word, suffix)-pair. This can be fixed by including the pair as a new suffix and
    /// adding more measurements to distinguish the offending rows.
    pub fn make_consistent(&mut self) {
        while let Some(violation) = self.find_consistent_property_violation() {
            self.fix_consistent_property_violation(violation);
        }
    }

    fn find_consistent_property_violation(&mut self) -> Option<ConsistencyViolation> {
        // offending rows must be in the same graph node
        for state_id in self.graph.node_indices() {
            let state_rows = self.graph.node_weight(state_id).unwrap();
            for rows in state_rows.iter().cartesian_product(state_rows.iter()) {
                // find mismatching observation in candidate rows
                for (word_id, suffix_id) in self.words.keys().cartesian_product(self.suffixes.keys()) {
                    // find successors when applying the word and compare suffix observation
                    let target_row_ids = match (
                        self.candidate_transitions.get(&(*rows.0, word_id)).copied(),
                        self.candidate_transitions.get(&(*rows.1, word_id)).copied(),
                    ) {
                        (Some(id_0), Some(id_1)) => (id_0, id_1),
                        _ => continue,
                    };

                    let target_rows =
                        (&self.observation_rows[target_row_ids.0], &self.observation_rows[target_row_ids.1]);

                    match target_rows
                        .0
                        .get_observation(suffix_id)
                        .compare(&target_rows.1.get_observation(suffix_id))
                    {
                        ObservationComparison::NotEqual => {
                            // contradiction found
                            return Some(ConsistencyViolation::new(
                                (target_row_ids.0, target_row_ids.1),
                                word_id,
                                suffix_id,
                            ));
                        }
                        ObservationComparison::Equal | ObservationComparison::Undetermined => {
                            // if we don't know for sure whether they are contradicting we
                            // do nothing
                        }
                    }
                }
            }
        }

        None
    }

    fn fix_consistent_property_violation(
        &mut self,
        violation: ConsistencyViolation,
    ) {
        // we add the offending word to the suffix list
        let offending_suffix = &self.suffixes[violation.suffix_id];
        let mut new_suffix = Vec::with_capacity(offending_suffix.len() + 1);
        new_suffix.push(violation.word_id);
        new_suffix.extend(offending_suffix.iter());
        let new_suffix_id = self.add_suffix(new_suffix);

        for violation_row in [violation.rows.0, violation.rows.1] {
            let predecessor = self.observation_rows[violation_row]
                .predecessor_row
                .unwrap();

            self.add_observation(
                predecessor,
                new_suffix_id,
                self.observation_rows[violation_row]
                    .get_observation(violation.suffix_id)
                    .unwrap()
                    .clone(),
            )
            .unwrap();
        }
    }

    fn fix_closed_property_violation(
        &mut self,
        candidate_id: ObservationRowId,
    ) {
        self.promote_candidate_to_state(candidate_id);
    }

    fn find_closed_property_violation(&self) -> Option<ObservationRowId> {
        for candidate_id in self.candidate_transitions.values() {
            if self.candidate_violates_closed_property(*candidate_id) {
                return Some(*candidate_id);
            }
        }
        None
    }

    fn candidate_violates_closed_property(
        &self,
        candidate_id: ObservationRowId,
    ) -> bool {
        let candidate_row = &self.observation_rows[candidate_id];
        for (_, row) in self
            .observation_rows
            .iter()
            .filter(|(row_id, _)| self.row_to_state.contains_key(row_id))
        {
            if matches!(
                row.compare_observations(candidate_row),
                ObservationComparison::Equal | ObservationComparison::Undetermined
            ) {
                // The candidate does not violate the closed property, since it is equal (or maybe equal) to
                // an existing state. If maybe equal, then we must wait for further observations to determine
                // if the closed property is violated or not.
                return false;
            }
        }
        // We found no state that is equal or undetermined when comparing to the candidate.
        // Therefore, the candidate violates the closed property, since it is not equal to any state.
        true
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
        let mut previous_word = None;
        while let Some(word) = prefix.next() {
            if let Some(nex_row) = self.state_transitions.get(&(current_row, word)) {
                current_row = *nex_row;
                previous_word = Some(word);
            } else {
                previous_word = Some(word);
                break;
            }
        }

        // current_row is now the last row that exists for this prefix.
        // We need to now add all prefix rows to the state_table except
        // for the last one. This is added to the candidates.

        // Iterate over the remaining words except the last.
        let mut remaining_prefix = previous_word.iter().copied().chain(prefix).peekable();
        while let Some(word) = remaining_prefix.next()
            && remaining_prefix.peek().is_some()
        {
            let candidate = self.candidate_transitions[&(current_row, word)];
            self.promote_candidate_to_state(candidate);
        }
        // We have now added all word that belong to the prefix to the state table.
        // The last word is automatically contained in the candidate table.
    }

    fn promote_candidate_to_state(
        &mut self,
        candidate_id: ObservationRowId,
    ) {
        let row = &self.observation_rows[candidate_id];
        let Some(predecessor_row) = row.predecessor_row else {
            panic!("Candidate states must have a predecessor state.")
        };

        // move up ^~^
        // Move the candidate from the candidate_transitions table into the
        // state_transitions table
        self.state_transitions
            .insert((predecessor_row, row.transition_word), candidate_id);
        self.candidate_transitions
            .remove(&(predecessor_row, row.transition_word));

        self.row_to_state.insert(candidate_id, Default::default());
        self.update_graph_for_new_observation(candidate_id);

        // Add the new candidates that are now possible due to the new state.
        self.add_missing_candidates();
    }

    fn update_graph_for_new_observation(
        &mut self,
        modified: ObservationRowId,
    ) {
        // We want to find all graph nodes that contain a row that is equal to the candidate's row.
        // We can then add the candidate's row to the rows in the node.
        let mut added_to_existing = false;
        for state in self.graph.node_indices() {
            let modified_row = &self.observation_rows[modified];
            let state_rows = self
                .graph
                .node_weight(state)
                .expect("We got the key from the graph");
            let all_rows_similar = state_rows.iter().all(|row_id| {
                match self.observation_rows[*row_id].compare_observations(&modified_row) {
                    ObservationComparison::Equal | ObservationComparison::Undetermined => true,
                    _ => false,
                }
            });
            if all_rows_similar {
                added_to_existing = true;
                let state_rows = self
                    .graph
                    .node_weight_mut(state)
                    .expect("We got the key from the graph");
                state_rows.insert(modified);
                // Add the state we found to the row_to_state map entry for this row.
                if let Some(predecessor_row) = modified_row.predecessor_row {
                    self.update_graph_for_new_transition(
                        state,
                        (predecessor_row, modified_row.transition_word, modified),
                    );
                }
            } else {
                if let Some(predecessor_row) = modified_row.predecessor_row {
                    let predecessor_states = &self.row_to_state[&predecessor_row];
                    for predecessor_state in predecessor_states.iter() {
                        if let Some(edge) = self.graph.find_edge(*predecessor_state, state) {
                            self.graph.remove_edge(edge);
                        }
                    }
                }

                let state_rows = self
                    .graph
                    .node_weight_mut(state)
                    .expect("We got the key from the graph");
                state_rows.remove(&modified);
                self.row_to_state.get_mut(&modified).unwrap().remove(&state);
            }
        }

        if !added_to_existing {
            let row = &self.observation_rows[modified];
            let new_state = self.graph.add_node(BTreeSet::from([modified]));
            if let Some(predecessor_row) = row.predecessor_row {
                self.update_graph_for_new_transition(new_state, (predecessor_row, row.transition_word, modified));
            }
        }
    }

    fn update_graph_for_new_transition(
        &mut self,
        target_state: NodeIndex,
        transition: (ObservationRowId, WordId, ObservationRowId),
    ) {
        let (predecessor_row, word, target_row) = transition;
        self.row_to_state
            .get_mut(&target_row)
            .unwrap()
            .insert(target_state);
        let predecessors = self
            .row_to_state
            .get(&predecessor_row)
            .expect("This must have been inserted before");

        for predecessor in predecessors.iter().copied() {
            self.graph.add_edge(predecessor, target_state, word);
        }
    }
}

impl<T> Display for StateLearner<T>
where
    T: Debug,
{
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        writeln!(f, "StateLearner:")?;
        write!(f, "Suffixes:")?;
        for suffix in self.suffixes.values() {
            write!(f, " {:?}, ", suffix)?;
        }
        write!(f, "\nRows:\n")?;
        for (row_id, row) in self.observation_rows.iter() {
            let mut row_string = String::new();
            if let Some(states) = self.row_to_state.get(&row_id) {
                if let Some(first_state) = states.first() {
                    write!(row_string, "{:<16}", format!("<{first_state:?}>"))?;
                }
            } else {
                write!(row_string, "{:<16}", "<candidate>")?;
            }
            row.fmt_debug_row(&format!("{}", row_id), &mut row_string, &self.suffixes)?;
            if let Some(states) = self.row_to_state.get(&row_id) {
                writeln!(f, "{}", ansi_term::Colour::Green.paint(row_string))?;
                for state in states.iter().skip(1) {
                    writeln!(f, "{:<16}", ansi_term::Colour::Green.paint(format!("<{state:?}>")))?;
                }
            } else {
                writeln!(f, "{}", row_string)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::graph::{
        StateLearner,
        WordId,
    };
    use slotmap::SlotMap;
    use std::path::Path;

    #[test]
    fn it_works() {
        let mut words = SlotMap::<WordId, String>::default();
        let epsilon = words.insert(String::from(""));
        let word_ids = ["A", "B"]
            .into_iter()
            .map(|word| words.insert(String::from(word)))
            .collect::<Vec<_>>();
        let a = word_ids[0];
        let b = word_ids[1];
        let mut learner = StateLearner::<String>::new(epsilon);

        let initial_row = learner.initial_row();

        learner.add_suffixes(word_ids.iter().copied().map(|word_id| [word_id]));
        learner.add_suffixes([vec![word_ids[0], word_ids[1]]]);

        println!("{}", learner);

        let unobserved = learner.unobserved_suffixes(initial_row).collect::<Vec<_>>();

        learner
            .add_observation(initial_row, unobserved[0], String::from("test"))
            .unwrap();
        learner
            .add_observation(initial_row, unobserved[1], String::from("test2"))
            .unwrap();
        learner
            .add_observation(initial_row, unobserved[2], String::from("test"))
            .unwrap();
        learner
            .add_observation(initial_row, unobserved[3], String::from("test"))
            .unwrap();

        let candidate_row = learner
            .get_candidate_row_id(initial_row, word_ids[0])
            .unwrap();

        learner
            .add_observation(candidate_row, unobserved[0], String::from("test"))
            .unwrap();
        learner
            .add_observation(candidate_row, unobserved[1], String::from("test"))
            .unwrap();
        learner
            .add_observation(candidate_row, unobserved[2], String::from("test2"))
            .unwrap();
        learner
            .add_observation(candidate_row, unobserved[3], String::from("test"))
            .unwrap();

        println!("{}", learner);

        learner.make_closed();
        println!("{}", learner);

        let row_id = learner
            .get_candidate_row_id(candidate_row, word_ids[1])
            .unwrap();
        learner
            .add_observation(row_id, unobserved[1], String::from("test3"))
            .unwrap();

        println!("Rows with missing observations: {:?}", learner.rows_with_missing_observations().collect::<Vec<_>>());

        println!("{}", learner);
        learner.make_closed();
        learner.add_prefix([a, b, a, b]);
        println!("{}", learner);

        let row = learner.get_state_row_id_from_prefix([a, b, a]).unwrap();
        let suffix = learner.unobserved_suffixes(row).skip(1).next().unwrap();
        learner
            .add_observation(row, suffix, String::from("test4"))
            .unwrap();
        println!("{}", learner);

        // learner.add_suffixes([vec![word_ids[1], word_ids[0]]]);

        // println!("{}", learner);

        learner.export(Path::new("test.dot"));
    }
}
