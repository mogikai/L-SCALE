use crate::row::{
    ObservationRowComparison,
    ObservationRowId,
};
use crate::{
    ObservationValue,
    StateLearner,
    SuffixId,
    SymbolId,
};
use itertools::Itertools;
use log::{
    debug,
    info,
};

#[derive(Debug)]
struct ConsistencyViolation {
    rows: (ObservationRowId, ObservationRowId),
    symbol_id: SymbolId,
    suffix_id: SuffixId,
}

impl ConsistencyViolation {
    fn new(
        rows: (ObservationRowId, ObservationRowId),
        symbol_id: SymbolId,
        suffix_id: SuffixId,
    ) -> Self {
        Self {
            rows,
            symbol_id,
            suffix_id,
        }
    }
}

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    /// The consistent property is violated if two equal rows exhibit different behavior under a
    /// certain (symbol, suffix)-pair. This can be fixed by including the pair as a new suffix and
    /// adding more measurements to distinguish the offending rows.
    pub fn make_consistent(&mut self) {
        while let Some(violation) = self.find_consistent_property_violation() {
            debug!("Found consistency violation: {violation:?}");
            self.fix_consistent_property_violation(violation);
        }
    }

    fn find_consistent_property_violation(&mut self) -> Option<ConsistencyViolation> {
        // offending rows must be in the same graph node
        for state_id in self.graph.node_indices() {
            let state_rows = self.graph.node_weight(state_id).unwrap();
            let state_rows_iter = state_rows
                .iter()
                .filter(|rid| self.observation_rows[**rid].is_state());
            for rows in state_rows_iter.clone().cartesian_product(state_rows_iter) {
                // find mismatching observation in candidate rows
                for symbol_id in self.symbols.keys() {
                    // find successors when applying the symbol and compare suffix observation
                    let target_row_ids = match (
                        self.transitions.get(&(*rows.0, symbol_id)).copied(),
                        self.transitions.get(&(*rows.1, symbol_id)).copied(),
                    ) {
                        (Some(id_0), Some(id_1)) => (id_0, id_1),
                        _ => continue,
                    };

                    let target_rows =
                        (&self.observation_rows[target_row_ids.0], &self.observation_rows[target_row_ids.1]);

                    if let ObservationRowComparison::NotEqual(witness) =
                        target_rows.0.compare_observations(target_rows.1)
                    {
                        // contradiction found
                        let violation =
                            ConsistencyViolation::new((target_row_ids.0, target_row_ids.1), symbol_id, witness);
                        return Some(violation);
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
        // we add the offending symbol to the suffix list
        let offending_suffix = &self.suffixes[violation.suffix_id];
        let mut new_suffix = Vec::with_capacity(offending_suffix.len() + 1);
        new_suffix.push(violation.symbol_id);
        new_suffix.extend(offending_suffix.iter());
        let new_suffix_id = self.add_suffix(new_suffix);

        for violation_row in [violation.rows.0, violation.rows.1] {
            let (predecessor, _) = self.observation_rows[violation_row]
                .predecessor_transition()
                .unwrap();

            self.add_observation(
                predecessor,
                new_suffix_id,
                self.observation_rows[violation_row]
                    .observation(violation.suffix_id)
                    .unwrap()
                    .clone(),
            )
            .unwrap();
        }
    }
}

#[cfg(test)]
mod test {
    use slotmap::SlotMap;

    use super::*;

    #[test]
    fn find_consistency_violations() {
        let symbols = ["A", "B", "C", "D"];

        let mut symbol_map = SlotMap::<SymbolId, String>::default();
        let symbol_ids = symbols
            .iter()
            .map(|symbol| symbol_map.insert(symbol.to_string()))
            .collect::<Vec<_>>();

        let mut learner = StateLearner::<String>::default();
        for symbol in symbol_ids.iter().copied() {
            learner.add_prefix([symbol]);
            learner.add_suffix([symbol]);
        }

        let rows = learner.observation_rows.keys().collect::<Vec<_>>();
        let suffixes = learner.suffixes.keys().collect::<Vec<_>>();

        learner
            .add_observation(rows[0], suffixes[0], "test0".into())
            .unwrap();
        learner
            .add_observation(rows[1], suffixes[0], "test0".into())
            .unwrap();
        learner.promote_candidate_to_state(rows[1]);
        let rows = learner.observation_rows.keys().collect::<Vec<_>>();

        // break consistency
        learner
            .add_observation(rows[2], suffixes[1], "test1".into())
            .unwrap();
        learner
            .add_observation(rows[6], suffixes[1], "test2".into())
            .unwrap();

        let violation = learner.find_consistent_property_violation().unwrap();
        assert_eq!(violation.rows, (rows[2], rows[6]));
        assert_eq!(violation.symbol_id, symbol_ids[1]);
        assert_eq!(violation.suffix_id, suffixes[1]);
    }

    #[test]
    fn fix_consistency_violations() {
        let symbols = ["A", "B", "C", "D"];

        let mut symbol_map = SlotMap::<SymbolId, String>::default();
        let symbol_ids = symbols
            .iter()
            .map(|symbol| symbol_map.insert(symbol.to_string()))
            .collect::<Vec<_>>();

        let mut learner = StateLearner::<String>::default();
        for symbol in symbol_ids.iter().copied() {
            learner.add_prefix([symbol]);
            learner.add_suffix([symbol]);
        }

        let rows = learner.observation_rows.keys().collect::<Vec<_>>();
        let suffixes = learner.suffixes.keys().collect::<Vec<_>>();

        learner
            .add_observation(rows[0], suffixes[0], "test0".into())
            .unwrap();
        learner
            .add_observation(rows[1], suffixes[0], "test0".into())
            .unwrap();
        learner.promote_candidate_to_state(rows[1]);
        let rows = learner.observation_rows.keys().collect::<Vec<_>>();

        // break consistency
        learner
            .add_observation(rows[2], suffixes[2], "test1".into())
            .unwrap();
        learner
            .add_observation(rows[6], suffixes[2], "test2".into())
            .unwrap();

        let violation = learner.find_consistent_property_violation().unwrap();
        assert_eq!(violation.rows, (rows[2], rows[6]));
        assert_eq!(violation.symbol_id, symbol_ids[1]);
        assert_eq!(violation.suffix_id, suffixes[2]);

        // fix it
        learner.fix_consistent_property_violation(violation);

        let new_suffix = learner.suffixes.values().last().unwrap();
        assert_eq!(new_suffix[0], symbol_ids[1]);
        assert_eq!(new_suffix[1], symbol_ids[2]);

        let observation_0 = learner.observation_rows[rows[0]]
            .observation(learner.suffixes.keys().last().unwrap())
            .unwrap();
        let observation_1 = learner.observation_rows[rows[1]]
            .observation(learner.suffixes.keys().last().unwrap())
            .unwrap();

        assert_eq!(observation_0, "test1");
        assert_eq!(observation_1, "test2");
    }
}
