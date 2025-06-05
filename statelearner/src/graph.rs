use itertools::Itertools;
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
        let pred_str = self.predecessor_row.map(|pred| format!("{:?}", pred)).unwrap_or("n/a".into());
        write!(f, "{:<45}", format!("({:?} --[{:?}]-->) ", pred_str, self.transition_word))?;
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

struct StateLearner<T> {
    /// Contains a row of observations for each state or candidate state.
    observation_rows: SlotMap<ObservationRowId, ObservationRow<T>>,
    /// The graph that contains the states and the edges between them.
    /// The node 'weights' are the corresponding observation rows (or rather their ids)
    /// and the edge 'weights' are the corresponding words (or rather their ids) that trigger the transition.
    graph: DiGraph<Vec<ObservationRowId>, WordId>,

    /// A reverse lookup map for states.
    row_to_state: HashMap<ObservationRowId, Vec<NodeIndex>>,

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
        let mut graph = DiGraph::<Vec<ObservationRowId>, WordId>::default();
        let mut observation_rows = SlotMap::<ObservationRowId, ObservationRow<T>>::default();
        let initial_row = observation_rows.insert(ObservationRow {
            predecessor_row: None,
            transition_word: epsilon,
            observations: Default::default(),
        });

        let mut row_to_state = HashMap::<ObservationRowId, Vec<NodeIndex>>::default();
        let initial_state = graph.add_node(vec![initial_row]);
        row_to_state.insert(initial_row, vec![initial_state]);

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
        slf.add_words(once(epsilon));
        slf.add_missing_candidates();
        slf
    }

    pub fn initial_row(&self) -> ObservationRowId {
        self.initial_row
    }

    pub fn get_state_row_id(
        &self,
        prefix: impl Iterator<Item = WordId>,
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

    pub fn get_candidate_row_id(
        &self,
        predecessor: ObservationRowId,
        word: WordId,
    ) -> Option<ObservationRowId> {
        self.candidate_transitions
            .get(&(predecessor, word))
            .copied()
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
                    for word in suffix.iter() {
                        self.words.insert(*word, ());
                    }
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

    pub fn state_observation_rows(
        &self,
        state: NodeIndex,
    ) -> &[ObservationRowId] {
        self.graph.node_weight(state).unwrap()
    }

    pub fn rows_with_missing_observations(&self) -> impl Iterator<Item = ObservationRowId> {
        self.observation_rows
            .keys()
            .filter(|row| self.unobserved_suffixes(*row).next().is_some())
    }

    pub fn states_with_missing_observations(&self) -> impl Iterator<Item = ObservationRowId> {
        self.rows_with_missing_observations().filter(|row_id| {
            self.row_to_state.contains_key(&row_id)
        })
    }

    pub fn candidates_with_missing_observations(&self) -> impl Iterator<Item = ObservationRowId> {
        self.rows_with_missing_observations().filter(|row_id| {
            !self.row_to_state.contains_key(&row_id)
        })
    }

    fn add_missing_candidates(&mut self) {
        let new_candidates = self
            .observation_rows
            .keys()
            .filter(|row_id| self.row_to_state.contains_key(row_id))
            .cartesian_product(self.words.keys())
            .filter_map(|candidate| {
                if self.candidate_transitions.contains_key(&candidate) {
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
        Ok(())
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
        let Some(predecessor_row) = row.predecessor_row else {
            panic!("Candidate states must have a predecessor state.")
        };

        // move up ^~^
        self.state_transitions
            .insert((predecessor_row, row.transition_word), candidate_id);
        self.candidate_transitions
            .remove(&(predecessor_row, row.transition_word));

        let new_state = self.graph.add_node(vec![candidate_id]);
        self.row_to_state.insert(candidate_id, vec![new_state]);

        let predecessors = self
            .row_to_state
            .get(&predecessor_row)
            .expect("This must have been inserted before");

        for predecessor in predecessors.iter().copied() {
            self.graph
                .add_edge(predecessor, new_state, row.transition_word);
        }

        // Add the new candidates that are now possible due to the new state.
        self.add_missing_candidates();
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
            row.fmt_debug_row(&mut row_string, &self.suffixes)?;
            if let Some(states) = self.row_to_state.get(&row_id) {
                writeln!(f, "{}", ansi_term::Colour::Green.paint(row_string))?;
                for state in states.iter().skip(1) {
                    writeln!(f, "{:<16}", format!("<{state:?}>"))?;
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
        ObservationRowId,
        StateLearner,
        WordId,
    };
    use petgraph::graph::NodeIndex;
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

        let initial_row = learner.initial_row();

        learner.add_words(word_ids.iter().copied());
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
        println!("{}", learner);

        // learner.add_suffixes([vec![word_ids[1], word_ids[0]]]);

        // println!("{}", learner);

        learner.export(Path::new("test.dot"));
    }
}
