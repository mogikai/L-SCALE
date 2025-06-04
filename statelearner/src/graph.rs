use petgraph::dot::Dot;
use petgraph::graph::{
    DiGraph,
    NodeIndex,
};
use petgraph::visit::NodeRef;
use slotmap::{
    SecondaryMap,
    SlotMap,
    new_key_type,
};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
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

trait ObservationValue: PartialEq {}

#[derive(Default, Debug)]
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

new_key_type! { struct ObservationRowId; }
new_key_type! { struct SuffixId; }
new_key_type! { struct WordId; }

/// A row of observations for a state or candidate state.
/// By default, values in columns that have not been filled are defined as [Observation::Missing].
#[derive(Debug)]
struct ObservationRow<T> {
    /// The state that is the predecessor of this row.
    /// This new row is arrived at by applying the transition word to the predecessor state.
    predecessor_state: Option<NodeIndex>,
    /// The word that is applied to the predecessor row to arrive at this row.
    transition_word: WordId,
    /// The observations when applying the corresponding suffix to the
    /// target in the state represented by this row.
    observations: SecondaryMap<SuffixId, Observation<T>>,
}

impl<T> ObservationRow<T> {
    pub fn new_candidate(
        predecessor: NodeIndex,
        word: WordId,
    ) -> Self {
        Self {
            predecessor_state: Some(predecessor),
            transition_word: word,
            observations: Default::default(),
        }
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
        f: &mut impl Write,
        suffixes: &SlotMap<SuffixId, Vec<WordId>>,
    ) -> std::fmt::Result {
        write!(f, "{:<40}", format!("({:?} --[{:?}]-->) ", self.predecessor_state, self.transition_word))?;
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
        write!(f, "\n")?;
        Ok(())
    }
}

struct StateLearner<T> {
    /// Contains a row of observations for each state or candidate state.
    observation_rows: SlotMap<ObservationRowId, ObservationRow<T>>,
    /// The graph that contains the states and the edges between them.
    /// The node 'weights' are the corresponding observation rows (or rather their ids)
    /// and the edge 'weights' are the corresponding words (or rather their ids) that trigger the transition.
    graph: DiGraph<ObservationRowId, WordId>,

    /// A reverse lookup map for states.
    row_to_state: HashMap<ObservationRowId, NodeIndex>,

    /// A set of candidate states. A candidate state is a state that can be arrived at
    /// by applying a word to an existing state. The number of candidates is thus at most
    /// |V| * |W| where |V| is the number of states and |W| is the number of words.
    /// A candidate state is identified by its corresponding observation row.
    candidates: HashMap<(NodeIndex, WordId), ObservationRowId>,

    /// All suffixes that we have seen so far. A suffix is a list of words applied in a certain state.
    suffixes: SlotMap<SuffixId, Vec<WordId>>,
    /// Reverse lookup map for suffixes. This is used to check if a suffix was already added to the learner.
    suffix_lookup: HashMap<Vec<WordId>, SuffixId>,

    initial_state: NodeIndex,

    /// The id of the empty word, i.e., a word that consists of zero characters.
    /// This can be considered as simply running the target without applying any input.
    epsilon: WordId,
}

impl<T> StateLearner<T> {
    pub fn new(epsilon: WordId) -> Self {
        let mut graph = DiGraph::<ObservationRowId, WordId>::default();
        let mut observation_rows = SlotMap::<ObservationRowId, ObservationRow<T>>::default();
        let initial_observations = observation_rows.insert(ObservationRow {
            predecessor_state: None,
            transition_word: epsilon,
            observations: Default::default(),
        });

        let mut row_to_state = HashMap::<ObservationRowId, NodeIndex>::default();
        let initial_state = graph.add_node(initial_observations);
        row_to_state.insert(initial_observations, initial_state);

        let mut slf = Self {
            observation_rows,
            graph,
            row_to_state,
            candidates: Default::default(),
            suffixes: Default::default(),
            suffix_lookup: Default::default(),
            initial_state,
            epsilon,
        };
        slf.add_words(once(epsilon));
        slf.add_missing_candidates();
        slf
    }

    pub fn initial_state(&self) -> NodeIndex {
        self.initial_state
    }

    /// Add the provided words into the state learner.
    /// Any existing words will be skipped.
    pub fn add_words(
        &mut self,
        words: impl IntoIterator<Item = WordId>,
    ) {
        self.add_suffixes(words.into_iter().map(|word| vec![word]));
    }

    /// Add the provided suffixes into the state learner.
    /// Any existing suffixes will be skipped.
    pub fn add_suffixes(
        &mut self,
        suffixes: impl IntoIterator<Item = Vec<WordId>>,
    ) {
        suffixes
            .into_iter()
            .for_each(|suffix| match self.suffix_lookup.entry(suffix.clone()) {
                Entry::Occupied(_) => { /* The suffix was already added. */ }
                Entry::Vacant(entry) => {
                    entry.insert(self.suffixes.insert(suffix));
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

    pub fn state_observation_row(
        &self,
        state: NodeIndex,
    ) -> ObservationRowId {
        *self.graph.node_weight(state).unwrap()
    }

    pub fn rows_with_missing_observations(&self) -> impl Iterator<Item = ObservationRowId> {
        self.observation_rows
            .keys()
            .filter(|row| self.unobserved_suffixes(*row).next().is_some())
    }

    pub fn states_with_missing_observations(&self) -> impl Iterator<Item = NodeIndex> {
        self.graph.node_indices().filter(move |state| {
            self.unobserved_suffixes(self.state_observation_row(*state))
                .next()
                .is_some()
        })
    }

    pub fn state_candidates_with_missing_observations(&self) -> impl Iterator<Item = (NodeIndex, WordId)> {
        self.graph
            .node_indices()
            .map(|state| {
                self.suffixes.values().filter_map(move |suffix| {
                    if suffix.len() == 1 {
                        Some((state, suffix[0]))
                    } else {
                        None
                    }
                })
            })
            .flatten()
            .filter(|(state, word)| {
                if let Some(mut suffixes) = self
                    .candidates
                    .get(&(*state, *word))
                    .map(|row| self.unobserved_suffixes(*row))
                {
                    suffixes.next().is_some()
                } else {
                    // The candidate does not exist yet.
                    true
                }
            })
    }

    fn add_missing_candidates(&mut self) {
        for candidate in self
            .graph
            .node_indices()
            .map(|state| {
                self.suffixes.values().filter_map(move |suffix| {
                    if suffix.len() == 1 {
                        Some((state, suffix[0]))
                    } else {
                        None
                    }
                })
            })
            .flatten()
        {
            if let Entry::Vacant(entry) = self.candidates.entry(candidate) {
                // If the candidate does not exist yet, we add a new observation row for it and add
                // it to the candidates map.
                let row_id = self
                    .observation_rows
                    .insert(ObservationRow::new_candidate(candidate.0, candidate.1));
                entry.insert(row_id);
            }
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
        add_observation(observations, suffix_id, observation)
    }

    /// Add an observation to the state for the provided suffix
    pub fn add_state_observation(
        &mut self,
        state: NodeIndex,
        suffix_id: SuffixId,
        observation: T,
    ) -> StateLearnerResult {
        let observation_row_id = *self.graph.node_weight(state).unwrap();
        self.add_observation(observation_row_id, suffix_id, observation)
    }

    /// Add an observation to the candidate state identified by the state it branches off from and the corresponding
    /// word that triggers the transition.
    pub fn add_state_candidate_observation(
        &mut self,
        state: NodeIndex,
        word: WordId,
        suffix_id: SuffixId,
        observation: T,
    ) -> StateLearnerResult {
        let row_id = match self.candidates.entry((state, word)) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let row_id = self
                    .observation_rows
                    .insert(ObservationRow::new_candidate(state, word));
                entry.insert(row_id);
                row_id
            }
        };
        self.add_observation(row_id, suffix_id, observation)
    }

    pub fn make_closed(&mut self) {
        while let Some(candidate_id) = self.find_closed_property_violation() {
            self.fix_closed_property_violation(candidate_id);
        }
    }

    fn fix_closed_property_violation(
        &mut self,
        candidate_id: ObservationRowId,
    ) {
        let row = &self.observation_rows[candidate_id];
        let Some(predecessor) = row.predecessor_state else {
            panic!("Candidate states must have a predecessor state.")
        };
        let new_state = self.graph.add_node(candidate_id);
        self.row_to_state.insert(candidate_id, new_state);
        self.graph
            .add_edge(predecessor, new_state, row.transition_word);
        self.candidates.remove(&(predecessor, row.transition_word));
        // Add the new candidates that are now possible due to the new state.
        self.add_missing_candidates();
    }

    fn find_closed_property_violation(&self) -> Option<ObservationRowId> {
        for candidate_id in self.candidates.values() {
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
        for state_id in self.graph.node_weights() {
            let state_row = &self.observation_rows[*state_id];
            if matches!(
                state_row.compare_observations(candidate_row),
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
}

fn add_observation<T>(
    observations: &mut SecondaryMap<SuffixId, Observation<T>>,
    suffix_id: SuffixId,
    observation: T,
) -> StateLearnerResult
where
    T: ObservationValue,
{
    if let Some(old_observation) = observations.get(suffix_id) {
        if let Observation::Observed(old_observation) = old_observation {
            if old_observation != &observation {
                Err(StateLearnerError::ConflictingObservation)?
            }
        }
    }
    observations.insert(suffix_id, Observation::Observed(observation));
    Ok(())
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
            if let Some(state) = self.row_to_state.get(&row_id) {
                write!(row_string, "{:<16}", format!("<{state:?}>"))?;
            } else {
                write!(row_string, "{:<16}", "<candidate>")?;
            }
            row.fmt_debug_row(&mut row_string, &self.suffixes)?;
            if self.row_to_state.get(&row_id).is_some() {
                write!(f, "{}", ansi_term::Colour::Green.paint(row_string))?;
            } else {
                write!(f, "{}", row_string)?;
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
        let mut learner = StateLearner::<String>::new(epsilon);

        let state = learner.initial_state();

        learner.add_words(word_ids.iter().copied());
        learner.add_suffixes([vec![word_ids[0], word_ids[1]]]);

        println!("{}", learner);

        let unobserved = learner
            .unobserved_suffixes(learner.state_observation_row(state))
            .collect::<Vec<_>>();
        for suffix in unobserved.iter() {
            learner
                .add_state_observation(state, *suffix, String::from("test"))
                .unwrap();
        }
        learner
            .add_state_candidate_observation(state, word_ids[0], unobserved[2], String::from("test2"))
            .unwrap();

        learner.make_closed();

        println!("Rows with missing observations: {:?}", learner.rows_with_missing_observations().collect::<Vec<_>>());

        println!("{}", learner);

        learner.add_suffixes([vec![word_ids[1], word_ids[0]]]);

        println!("{}", learner);

        learner.export(Path::new("test.dot"));
    }
}
