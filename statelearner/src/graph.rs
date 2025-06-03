use petgraph::dot::Dot;
use petgraph::graph::{
    DiGraph,
    NodeIndex,
};
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
    Formatter,
};
use std::fs::File;
use std::io::Write;
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
    /// The epsilon input (i.e., no input at all) is represented by a [None] value.
    transition_word: Option<WordId>,
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
            transition_word: Some(word),
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
        f: &mut Formatter<'_>,
        suffixes: &SlotMap<SuffixId, Vec<WordId>>,
    ) -> std::fmt::Result {
        write!(f, "({:?} --[{:?}]-->) ", self.predecessor_state, self.transition_word)?;
        write!(f, "")?;
        for suffix_id in suffixes.keys() {
            write!(f, "{:?}, ", self.observations.get(suffix_id))?;
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
    graph: DiGraph<ObservationRowId, Option<WordId>>,

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
}

impl<T> StateLearner<T> {
    pub fn new() -> Self {
        let mut graph = DiGraph::<ObservationRowId, Option<WordId>>::default();
        let mut observation_rows = SlotMap::<ObservationRowId, ObservationRow<T>>::default();
        let initial_observations = observation_rows.insert(ObservationRow {
            predecessor_state: None,
            transition_word: None,
            observations: Default::default(),
        });

        let initial_state = graph.add_node(initial_observations);

        Self {
            observation_rows,
            graph,
            candidates: Default::default(),
            suffixes: Default::default(),
            suffix_lookup: Default::default(),
            initial_state,
        }
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
            })
    }

    /// Returns an iterator over all suffixes that have missing observations
    pub fn unobserved_suffixes(
        &self,
        state: NodeIndex,
    ) -> impl Iterator<Item = SuffixId> {
        let observation_row_id = self.graph.node_weight(state).unwrap();
        self.suffixes.keys().filter(move |suffix_id| {
            matches!(
                self.observation_rows[*observation_row_id]
                    .observations
                    .get(*suffix_id)
                    .unwrap_or(&Observation::Missing),
                Observation::Missing
            )
        })
    }

    pub fn states_with_missing_observations(&self) -> impl Iterator<Item = NodeIndex> {
        self.graph
            .node_indices()
            .filter(move |state| self.unobserved_suffixes(*state).next().is_some())
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
        file.write_fmt(format_args!("{:?}", dot)).unwrap();
    }
}

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    /// Add an observation to the state for the provided suffix
    pub fn add_observation(
        &mut self,
        state: NodeIndex,
        suffix_id: SuffixId,
        observation: T,
    ) -> StateLearnerResult {
        let observation_row_id = self.graph.node_weight(state).unwrap();
        let observations = &mut self.observation_rows[*observation_row_id].observations;
        add_observation(observations, suffix_id, observation)
    }

    /// Add an observation to the candidate state identified by the state it branches off from and the corresponding
    /// word that triggers the transition.
    pub fn add_candidate_observation(
        &mut self,
        state: NodeIndex,
        word: WordId,
        suffix_id: SuffixId,
        observation: T,
    ) -> StateLearnerResult {
        match self.candidates.entry((state, word)) {
            Entry::Occupied(mut entry) => {
                let observations = &mut self.observation_rows[*entry.get()].observations;
                add_observation(observations, suffix_id, observation)?;
            }
            Entry::Vacant(entry) => {
                let row_id = self
                    .observation_rows
                    .insert(ObservationRow::new_candidate(state, word));
                let observations = &mut self.observation_rows[row_id].observations;
                add_observation(observations, suffix_id, observation)?;
                entry.insert(row_id);
            }
        }
        Ok(())
    }

    fn fix_closed_property(&mut self) {}

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
        write!(f, "\nRows:")?;
        for row in self.observation_rows.values() {
            row.fmt_debug_row(f, &self.suffixes)?;
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
        let word_ids = ["A", "B", "C", "D"]
            .into_iter()
            .map(|word| words.insert(String::from(word)))
            .collect::<Vec<_>>();
        let mut learner = StateLearner::<String>::new();

        let state = learner.initial_state();

        learner.add_words(word_ids.iter().copied());
        learner.add_suffixes([vec![word_ids[0], word_ids[1]]]);

        println!("{}", learner);

        let unobserved = learner.unobserved_suffixes(state).collect::<Vec<_>>();
        println!("Unobserved suffixes: {:?}", unobserved);
        learner
            .add_observation(state, unobserved[2], String::from("test"))
            .unwrap();
        println!("Unobserved suffixes: {:?}", learner.unobserved_suffixes(state).collect::<Vec<_>>());

        println!(
            "States with missing observations: {:?}",
            learner
                .states_with_missing_observations()
                .collect::<Vec<_>>()
        );
        println!(
            "State candidates with missing observations: {:?}",
            learner
                .state_candidates_with_missing_observations()
                .collect::<Vec<_>>()
        );

        println!("{}", learner);

        learner.export(Path::new("test.dot"));
    }
}
