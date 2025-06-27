use crate::row::{
    ObservationRowComparison,
    ObservationRowId,
};
use crate::{
    ObservationValue,
    StateLearner,
    SymbolId,
};
use fnv::FnvHashSet;
use petgraph::dot::{
    Config,
    Dot,
};
use petgraph::graph::{
    EdgeReference,
    NodeIndex,
};
use slotmap::SlotMap;
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
        self.rebuild_graph_transitions(true);

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

    pub fn update_graph_for_new_observation(
        &mut self,
        modified: ObservationRowId,
    ) {
        self.row_to_state
            .entry(modified)
            .or_insert(FnvHashSet::default());
        let added_to_existing = self.update_existing_states(modified);

        if !added_to_existing {
            self.graph.add_node(FnvHashSet::from_iter([modified]));
        }
    }

    fn rebuild_graph_transitions(
        &mut self,
        exclude_undetermined: bool,
    ) {
        self.graph.clear_edges();
        for ((from_row_id, symbol), to_row_id) in self.transitions.iter() {
            if exclude_undetermined {
                // exclude undetermined, non-self transitions
                let from_row = &self.observation_rows[*from_row_id];
                let to_row = &self.observation_rows[*to_row_id];
                match from_row.compare_observations(&to_row) {
                    ObservationRowComparison::Undetermined => {
                        continue;
                    }
                    _ => {}
                }
            }

            for from_state in self.row_to_state[from_row_id].iter() {
                for to_state in self.row_to_state[to_row_id].iter() {
                    match self.graph.find_edge(*from_state, *to_state) {
                        Some(edge) => {
                            self.graph.edge_weight_mut(edge).unwrap().insert(*symbol);
                        }
                        None => {
                            let mut symbols: FnvHashSet<SymbolId> = Default::default();
                            symbols.insert(*symbol);
                            self.graph.add_edge(*from_state, *to_state, symbols);
                        }
                    }
                }
            }
        }
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
                self.row_to_state.get_mut(&modified).unwrap().insert(state);
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ObservationComparisonResult;
    impl ObservationValue for String {
        fn compare(
            &self,
            other: &Self,
        ) -> ObservationComparisonResult {
            if self.eq(other) {
                ObservationComparisonResult(0)
            } else {
                ObservationComparisonResult(1)
            }
        }
    }

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

        learner.make_closed();
        learner.make_consistent();
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
        learner.make_closed();
        learner.make_consistent();
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

        learner.export(Path::new("test.dot"), &symbols);
    }
}
