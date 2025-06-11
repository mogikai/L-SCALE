use crate::ovservation::Observation;
use crate::row::{
    ObservationRowComparison,
    ObservationRowId,
};
use crate::{
    ObservationValue,
    StateLearner,
    SuffixId,
    WordId,
};
use fnv::FnvHashSet;
use itertools::Itertools;
use petgraph::dot::Dot;
use petgraph::graph::{
    EdgeReference,
    NodeIndex,
};
use slotmap::{
    Key,
    SlotMap,
};
use std::fmt::{
    Debug,
    Display,
    Write,
};
use std::fs::File;
use std::ops::Index;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateLearnerError {
    #[error("Conflicting observations. Setting an observation to a different value for a suffix is not allowed.")]
    ConflictingObservation,
    #[error("Unknown error")]
    Unknown,
}

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

    pub fn export<W>(
        &self,
        file: &Path,
        words: &SlotMap<WordId, W>,
    ) where
        W: Display,
    {
        let format_edge = &|_, edge: EdgeReference<FnvHashSet<WordId>, _>| {
            let mut label = String::new();
            for word_id in edge.weight().iter().copied() {
                write!(label, "{}, ", words[word_id]).unwrap();
            }
            format!("label = \"{}\"", label)
        };

        let format_node = &|_, (node_index, node_rows): (NodeIndex, &FnvHashSet<ObservationRowId>)| {
            format!("label = \"Node {} ({})\"", node_index.index(), node_rows.len())
        };

        let dot = Dot::with_attr_getters(&self.graph, &[], format_edge, format_node);
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
            if let Some(nex_row) = self.state_transitions.get(&(current_row, word)) {
                current_row = *nex_row;
            } else {
                if prefix.peek().is_none() {
                    // If this is the last word in the prefix we don't want to promote the candidate.
                    break;
                }
                // We want to promote all intermediate candidates to states.
                let candidate = self.candidate_transitions[&(current_row, word)];
                self.promote_candidate_to_state(candidate);
            }
        }
    }

    pub fn update_graph_for_new_observation(
        &mut self,
        modified: ObservationRowId,
    ) {
        self.row_to_state
            .entry(modified)
            .or_insert(FnvHashSet::default());
        let added_to_existing = self.update_existing_states(modified);

        self.prune_eliminated_transitions();

        if !added_to_existing {
            self.create_new_state(modified);
        }
    }

    pub fn create_new_state(
        &mut self,
        modified: ObservationRowId,
    ) {
        let row = &self.observation_rows[modified];
        let new_state = self.graph.add_node(FnvHashSet::from_iter([modified]));
        if let Some((predecessor_row, transition_word)) = row.predecessor_transition() {
            self.update_graph_for_new_transition(new_state, (predecessor_row, transition_word, modified));
        }
    }

    fn prune_eliminated_transitions(&mut self) {
        // remove all eliminated transition words
        for edge_index in self.graph.edge_indices() {
            let (start, end) = self.graph.edge_endpoints(edge_index).unwrap();
            let start_rows = self
                .graph
                .node_weight(start)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>();
            let end_rows = self
                .graph
                .node_weight(end)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>();

            let edge_words = self.graph.edge_weight_mut(edge_index).unwrap();
            edge_words.retain(|transition_word| {
                for start_row in start_rows.iter().copied() {
                    for transitions in [&self.state_transitions, &self.candidate_transitions] {
                        if let Some(target) = transitions.get(&(start_row, *transition_word))
                            && end_rows.contains(target)
                        {
                            return true;
                        }
                    }
                }

                false
            });
        }

        // remove all edges for which there are no transition words
        self.graph.retain_edges(|graph, edge_index| {
            let transition_words = graph.edge_weight(edge_index).unwrap();
            !transition_words.is_empty()
        });
    }

    fn update_existing_states(
        &mut self,
        modified: ObservationRowId,
    ) -> bool {
        // We want to find all graph nodes that contain a row that is equal to the candidate's row.
        // We can then add the candidate's row to the rows in the node.
        let mut added_to_existing = false;
        for state in self.graph.node_indices() {
            if self.all_rows_similar(state, modified) {
                added_to_existing = true;
                self.graph
                    .node_weight_mut(state)
                    .expect("We got the key from the graph")
                    .insert(modified);
                // Add the state we found to the row_to_state map entry for this row.
                let modified_row = &self.observation_rows[modified];
                if let Some((predecessor_row, transition_word)) = modified_row.predecessor_transition() {
                    self.update_graph_for_new_transition(state, (predecessor_row, transition_word, modified));
                }
            } else {
                self.graph
                    .node_weight_mut(state)
                    .expect("We got the key from the graph")
                    .remove(&modified);
                self.row_to_state.get_mut(&modified).unwrap().remove(&state);
            }
        }
        added_to_existing
    }

    /// Compares all rows in the state against the modified row.
    /// If the modified row can be determined to be not equal to at
    /// least one row in the state, then this function returns false.
    /// In all other cases it returns true.
    fn all_rows_similar(
        &self,
        state: NodeIndex,
        modified_row: ObservationRowId,
    ) -> bool {
        self.graph
            .node_weight(state)
            .expect("We got the key from the graph")
            .iter()
            .copied()
            .all(|row_id| {
                match self.observation_rows[row_id].compare_observations(&self.observation_rows[modified_row]) {
                    ObservationRowComparison::Equal | ObservationRowComparison::Undetermined => true,
                    _ => false,
                }
            })
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
            let edge = self.graph.find_edge(predecessor, target_state);
            if let Some(edge) = edge {
                self.graph.edge_weight_mut(edge).unwrap().insert(word);
            } else {
                self.graph
                    .add_edge(predecessor, target_state, FnvHashSet::from_iter([word]));
            }
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
            if row.is_state() {
                if let Some(states) = self.row_to_state.get(&row_id)
                    && let Some(first_state) = states.iter().next()
                {
                    write!(row_string, "{:<16}", format!("<{first_state:?}>"))?;
                } else {
                    write!(row_string, "{:<16}", "<state>")?;
                }
            } else {
                write!(row_string, "{:<16}", "<candidate>")?;
            }
            row.fmt_debug_row(&format!("{}", row_id), &mut row_string, &self.suffixes)?;
            if row.is_state() {
                writeln!(f, "{}", ansi_term::Colour::Green.paint(row_string))?;
                if let Some(states) = self.row_to_state.get(&row_id) {
                    for state in states.iter().skip(1) {
                        writeln!(f, "{:<16}", ansi_term::Colour::Green.paint(format!("<{state:?}>")))?;
                    }
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
    use crate::graph::WordId;
    use crate::ovservation::ObservationValue;
    use crate::StateLearner;
    use slotmap::SlotMap;
    use std::path::Path;

    impl ObservationValue for String {}

    #[test]
    fn it_works() {
        let mut words = SlotMap::<WordId, String>::default();
        let word_ids = ["A", "B"]
            .into_iter()
            .map(|word| words.insert(String::from(word)))
            .collect::<Vec<_>>();
        let a = word_ids[0];
        let b = word_ids[1];
        let mut learner = StateLearner::<String>::new();

        let initial_row = learner.initial_row();

        learner.add_suffixes(word_ids.iter().copied().map(|word_id| [word_id]));
        // learner.add_suffixes([vec![word_ids[0], word_ids[1]]]);

        println!("{}", learner);

        let unobserved = learner.unobserved_suffixes(initial_row).collect::<Vec<_>>();

        learner
            .add_observation(initial_row, unobserved[0], String::from("test"))
            .unwrap();
        learner
            .add_observation(initial_row, unobserved[1], String::from("test2"))
            .unwrap();

        let candidate_row = learner
            .get_candidate_row_id(initial_row, word_ids[0])
            .unwrap();

        learner
            .add_observation(candidate_row, unobserved[0], String::from("test"))
            .unwrap();
        learner
            .add_observation(candidate_row, unobserved[1], String::from("test2"))
            .unwrap();

        println!("{}", learner);

        learner.make_closed();
        learner.make_consistent();
        learner.add_prefix([a, a]);
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
        learner.make_consistent();
        learner.add_prefix([a, b, a, b]);
        println!("{}", learner);

        let row = learner.get_state_row_id_from_prefix([a, b, a]).unwrap();
        let suffixes = learner.unobserved_suffixes(row).collect::<Vec<_>>();
        learner
            .add_observation(row, suffixes[0], String::from("test4"))
            .unwrap();
        learner
            .add_observation(row, suffixes[1], String::from("test4"))
            .unwrap();
        println!("{}", learner);

        learner.make_closed();
        learner.make_consistent();
        for row in learner.rows_with_missing_observations().collect::<Vec<_>>() {
            let suffixes = learner.unobserved_suffixes(row).collect::<Vec<_>>();
            for suffix in suffixes {
                learner
                    .add_observation(row, suffix, String::from("test"))
                    .unwrap();
            }
        }
        println!("{}", learner);

        // learner.add_suffixes([vec![word_ids[1], word_ids[0]]]);

        // println!("{}", learner);

        learner.export(Path::new("test.dot"), &words);
    }
}
