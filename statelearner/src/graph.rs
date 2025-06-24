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
use std::path::Path;

impl<T> StateLearner<T> {
    pub fn export<S>(
        &self,
        file: &Path,
        symbols: &SlotMap<SymbolId, S>,
    ) where
        S: Display,
    {
        self.export_with_formatter(file, symbols, |out, symbol| write!(out, "{}", symbol).unwrap());
    }

    pub fn export_with_formatter<S, F>(
        &self,
        file: &Path,
        symbols: &SlotMap<SymbolId, S>,
        symbol_formatter: F,
    ) where
        S: Display,
        F: Fn(&mut dyn Write, &S),
    {
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
}

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
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
        if let Some((predecessor_row, transition_symbol)) = row.predecessor_transition() {
            self.update_graph_for_new_transition(new_state, (predecessor_row, transition_symbol, modified));
        }
    }

    fn prune_eliminated_transitions(&mut self) {
        // remove all eliminated transition symbols
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

            let edge_symbols = self.graph.edge_weight_mut(edge_index).unwrap();
            edge_symbols.retain(|transition_symbol| {
                for start_row in start_rows.iter().copied() {
                    if let Some(target) = self.transitions.get(&(start_row, *transition_symbol))
                        && end_rows.contains(target)
                    {
                        return true;
                    }
                }

                false
            });
        }

        // remove all edges for which there are no transition symbols
        self.graph.retain_edges(|graph, edge_index| {
            let transition_symbols = graph.edge_weight(edge_index).unwrap();
            !transition_symbols.is_empty()
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
                if let Some((predecessor_row, transition_symbol)) = modified_row.predecessor_transition() {
                    self.update_graph_for_new_transition(state, (predecessor_row, transition_symbol, modified));
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
        transition: (ObservationRowId, SymbolId, ObservationRowId),
    ) {
        let (predecessor_row, symbol, target_row) = transition;
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
                self.graph.edge_weight_mut(edge).unwrap().insert(symbol);
            } else {
                self.graph
                    .add_edge(predecessor, target_state, FnvHashSet::from_iter([symbol]));
            }
        }
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
