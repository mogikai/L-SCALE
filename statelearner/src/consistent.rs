use crate::row::{
    ObservationRow,
    ObservationRowComparison,
    ObservationRowId,
};
use crate::{
    ObservationValue,
    StateLearner,
    SuffixId,
    SymbolId,
};
use log::debug;

#[derive(Debug)]
pub struct ConsistencyViolation {
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
    pub(crate) fn find_consistent_property_violation(&mut self) -> Option<ConsistencyViolation> {
        // offending rows must be in the same graph node
        for row_0 in self.lower_rows() {
            let Some(transition) = row_0.1.predecessor_transition() else {
                continue;
            };
            for row_1 in self.distinct_lower_rows_with_same_symbol(transition) {
                // row_0 and row_1 are two rows reachable by the same symbol but from a different predecessor
                let (predecessor_row_id_1, _) = row_1.1.predecessor_transition().unwrap();
                let predecessor_0 = &self.observation_rows[transition.0];
                let predecessor_1 = &self.observation_rows[predecessor_row_id_1];
                assert!(predecessor_0.is_upper() && predecessor_1.is_upper());
                // Skip not equal predecessors, since we do not have to check for any inconsistencies here.
                // Unequal predecessors cannot be further distinguished by the suffix.
                if predecessor_0
                    .compare_observations(predecessor_1)
                    .not_equal()
                {
                    continue;
                }
                // Now we can look for a conflict in the observations of row_0 and row_1
                if let ObservationRowComparison::NotEqual(witness) = row_0.1.compare_observations(row_1.1) {
                    let violation = ConsistencyViolation::new((row_0.0, row_1.0), transition.1, witness);
                    return Some(violation);
                }
            }
        }
        None
    }

    fn distinct_lower_rows_with_same_symbol(
        &self,
        transition: (ObservationRowId, SymbolId),
    ) -> impl Iterator<Item = (ObservationRowId, &ObservationRow<T>)> {
        self.observation_rows.iter().filter(move |(_, row)| {
            let Some((predecessor_row_id_1, transition_symbol_1)) = row.predecessor_transition() else {
                return false;
            };
            !row.is_upper() && transition.0 != predecessor_row_id_1 && transition.1 == transition_symbol_1
        })
    }

    fn lower_rows(&self) -> impl Iterator<Item = (ObservationRowId, &ObservationRow<T>)> {
        self.observation_rows
            .iter()
            .filter(|(_, row)| !row.is_upper())
    }

    pub(crate) fn fix_consistent_property_violation(
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

            let observation = self.observation_rows[violation_row]
                .observation(violation.suffix_id)
                .unwrap()
                .clone();
            debug!("Fixing consistency property by copying observation from {violation_row:?} to {predecessor:?}");
            self.add_observation(predecessor, new_suffix_id, observation)
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
        learner.promote_candidate_to_upper(rows[1]);
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
        learner.promote_candidate_to_upper(rows[1]);
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
