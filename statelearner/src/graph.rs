use crate::row::{
    ObservationRowComparison,
    ObservationRowId,
};
use crate::{
    ObservationValue,
    StateLearner,
    SymbolId,
};
use fnv::{
    FnvHashMap,
    FnvHashSet,
};
use log::info;
use petgraph::dot::{
    Config,
    Dot,
};
use petgraph::graph::{
    EdgeReference,
    NodeIndex,
};
use slotmap::SlotMap;
use std::collections::VecDeque;
use std::fmt::{
    Display,
    Write,
};
use std::fs::File;
use std::ops::Not;
use std::path::Path;

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    pub fn export<S>(
        &mut self,
        file: &Path,
        symbols: &SlotMap<SymbolId, S>,
    ) where
        S: Display,
    {
        self.export_with_formatter(file, symbols, |out, symbol| write!(out, "{}", symbol).unwrap());
    }

    pub fn export_with_formatter<S, F>(
        &mut self,
        file: &Path,
        symbols: &SlotMap<SymbolId, S>,
        symbol_formatter: F,
    ) where
        S: Display,
        F: Fn(&mut dyn Write, &S),
    {
        // self.rebuild_graph_transitions(true);
        self.rebuild_graph();
        info!("Current nodes in graph: {}", self.graph.node_count());

        let format_edge = &|_, edge: EdgeReference<FnvHashSet<SymbolId>, _>| {
            let mut label = String::new();
            for symbol_id in edge.weight().iter().copied() {
                symbol_formatter(&mut label, &symbols[symbol_id]);
                write!(label, " ").unwrap();
            }
            format!("label = \"{}\"", label)
        };

        let format_node = &|_, (node_index, node_rows): (NodeIndex, &FnvHashSet<ObservationRowId>)| {
            format!("label = \"Node {} ({})\"", node_index.index(), node_rows.len())
        };

        let dot =
            Dot::with_attr_getters(&self.graph, &[Config::NodeNoLabel, Config::EdgeNoLabel], format_edge, format_node);
        let mut file = File::options()
            .truncate(true)
            .write(true)
            .create(true)
            .open(file)
            .unwrap();
        use std::io::Write;
        file.write_fmt(format_args!("{:?}", dot)).unwrap();
    }

    pub fn rebuild_graph(&mut self) {
        self.graph.clear();
        let mut row_to_state = FnvHashMap::<ObservationRowId, FnvHashSet<NodeIndex>>::default();
        // Only upper rows create states.
        for (row_id, row) in self
            .observation_rows
            .iter()
            .filter(|(_, row)| row.is_upper())
        {
            let existing_states = self.collect_existing_states(row_id);
            if existing_states.is_empty() {
                // There are no existing states that match this row. Create a new one.
                let mut row_set = FnvHashSet::default();
                row_set.insert(row_id);
                let state = self.graph.add_node(row_set);
                row_to_state.entry(row_id).or_default().insert(state);
            } else {
                // There are existing states for this row. Add it to all states that match.
                for existing_state in existing_states {
                    self.graph
                        .node_weight_mut(existing_state)
                        .unwrap()
                        .insert(row_id);
                    row_to_state
                        .entry(row_id)
                        .or_default()
                        .insert(existing_state);
                }
            }
        }

        let remaining_rows = self
            .observation_rows
            .iter()
            .filter(|(_, row)| !row.is_upper())
            .map(|(row_id, _)| row_id);

        for row_id in remaining_rows {
            let existing_states = self.collect_existing_states(row_id);

            for existing_state in existing_states {
                self.graph
                    .node_weight_mut(existing_state)
                    .unwrap()
                    .insert(row_id);
                row_to_state
                    .entry(row_id)
                    .or_default()
                    .insert(existing_state);
            }
        }

        for ((from_row_id, symbol), to_row_id) in self.transitions.iter() {
            // exclude undetermined, non-self transitions
            let from_row = &self.observation_rows[*from_row_id];
            let to_row = &self.observation_rows[*to_row_id];
            if matches!(from_row.compare_observations(&to_row), ObservationRowComparison::Undetermined) {
                continue;
            }
            if !row_to_state.contains_key(from_row_id) || !row_to_state.contains_key(to_row_id) {
                continue;
            }
            for from_state in row_to_state[from_row_id].iter().copied() {
                for to_state in row_to_state[to_row_id].iter().copied() {
                    match self.graph.find_edge(from_state, to_state) {
                        Some(edge) => {
                            self.graph.edge_weight_mut(edge).unwrap().insert(*symbol);
                        }
                        None => {
                            let mut symbols = FnvHashSet::<SymbolId>::default();
                            symbols.insert(*symbol);
                            self.graph.add_edge(from_state, to_state, symbols);
                        }
                    }
                }
            }
        }
    }

    fn collect_existing_states(
        &self,
        row_id: ObservationRowId,
    ) -> Vec<NodeIndex> {
        // We want all states where the provided row is equal or undetermined to all other rows.
        self.graph
            .node_indices()
            .filter(|index| {
                let rows = self.graph.node_weight(*index).unwrap();
                rows.iter().all(|&state_row| {
                    !matches!(
                        &self.observation_rows[state_row].compare_observations(&self.observation_rows[row_id]),
                        ObservationRowComparison::NotEqual(_)
                    )
                })
            })
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    impl ObservationValue for String {}

    #[test]
    fn it_works() {
        let mut symbols = SlotMap::<SymbolId, String>::default();
        let symbol_ids = ["A", "B"]
            .into_iter()
            .map(|symbol| symbols.insert(String::from(symbol)))
            .collect::<Vec<_>>();
        let a = symbol_ids[0];
        let b = symbol_ids[1];
        let mut learner = StateLearner::<String>::default();

        let initial_row = learner.initial_row();

        learner.add_suffixes(symbol_ids.iter().copied().map(|symbol_ids| [symbol_ids]));

        println!("{}", learner);

        let unobserved = learner.unobserved_suffixes(initial_row).collect::<Vec<_>>();

        learner
            .add_observation(initial_row, unobserved[0], String::from("test"))
            .unwrap();
        learner
            .add_observation(initial_row, unobserved[1], String::from("test2"))
            .unwrap();

        let candidate_row = learner
            .get_candidate_row_id(initial_row, symbol_ids[0])
            .unwrap();

        learner
            .add_observation(candidate_row, unobserved[0], String::from("test"))
            .unwrap();
        learner
            .add_observation(candidate_row, unobserved[1], String::from("test2"))
            .unwrap();

        println!("{}", learner);

        learner.make_consistent_and_closed();
        learner.add_prefix([a, a]);
        println!("{}", learner);

        let row_id = learner
            .get_candidate_row_id(candidate_row, symbol_ids[1])
            .unwrap();
        learner
            .add_observation(row_id, unobserved[1], String::from("test3"))
            .unwrap();

        println!("Rows with missing observations: {:?}", learner.rows_with_missing_observations().collect::<Vec<_>>());

        println!("{}", learner);
        learner.make_consistent_and_closed();
        learner.add_prefix([a, b, a, b]);
        println!("{}", learner);

        let row = learner.get_row_from_prefix([a, b, a]).unwrap();
        let suffixes = learner.unobserved_suffixes(row).collect::<Vec<_>>();
        learner
            .add_observation(row, suffixes[0], String::from("test4"))
            .unwrap();
        learner
            .add_observation(row, suffixes[1], String::from("test4"))
            .unwrap();
        println!("{}", learner);

        learner.make_consistent_and_closed();
        for row in learner.rows_with_missing_observations().collect::<Vec<_>>() {
            let suffixes = learner.unobserved_suffixes(row).collect::<Vec<_>>();
            for suffix in suffixes {
                learner
                    .add_observation(row, suffix, String::from("test"))
                    .unwrap();
            }
        }
        println!("{}", learner);

        learner.export(Path::new("test.dot"), &symbols);
    }
}
