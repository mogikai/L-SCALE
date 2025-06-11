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
use itertools::Itertools;

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

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    /// The consistent property is violated if two equal rows exhibit different behavior under a
    /// certain (word, suffix)-pair. This can be fixed by including the pair as a new suffix and
    /// adding more measurements to distinguish the offending rows.
    pub fn make_consistent(&mut self) {
        while let Some(violation) = self.find_consistent_property_violation() {
            println!("Found consistent property violation: {:?}", violation);
            self.fix_consistent_property_violation(violation);
        }
    }

    fn find_consistent_property_violation(&mut self) -> Option<ConsistencyViolation> {
        // offending rows must be in the same graph node
        for state_id in self.graph.node_indices() {
            let state_rows = self.graph.node_weight(state_id).unwrap();
            for rows in state_rows.iter().cartesian_product(state_rows.iter()) {
                // find mismatching observation in candidate rows
                for word_id in self.words.keys() {
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

                    if let ObservationRowComparison::NotEqual(witness) =
                        target_rows.0.compare_observations(target_rows.1)
                    {
                        // contradiction found
                        return Some(ConsistencyViolation::new((target_row_ids.0, target_row_ids.1), word_id, witness));
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
