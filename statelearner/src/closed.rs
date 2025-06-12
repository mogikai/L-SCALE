use crate::row::{
    ObservationRowComparison,
    ObservationRowId,
};
use crate::{
    ObservationValue,
    StateLearner,
};

impl<T> StateLearner<T>
where
    T: ObservationValue,
{
    pub fn make_closed(&mut self) {
        while let Some(candidate_id) = self.find_closed_property_violation() {
            println!("Found closed property violation: {:?}", candidate_id);
            self.promote_candidate_to_state(candidate_id);
        }
    }

    fn find_closed_property_violation(&self) -> Option<ObservationRowId> {
        for candidate_id in self
            .transitions
            .values()
            .copied()
            .filter(|&row| !self.observation_rows[row].is_state())
        {
            if self.candidate_violates_closed_property(candidate_id) {
                return Some(candidate_id);
            }
        }
        None
    }

    fn candidate_violates_closed_property(
        &self,
        candidate_id: ObservationRowId,
    ) -> bool {
        // Closed property: For a given row in the candidate_transitions table, there must exist a row in
        // the state_transitions table that is equal to the candidate. This effectively means that when
        // applying a symbol to an existing prefix, we cannot "walk out of the state space".
        // For our case, since there may be missing observations, rows are considered equal if they have
        // missing observations. They may only be considered unequal if they have at least one differing position.
        let candidate_row = &self.observation_rows[candidate_id];
        for row in self.observation_rows.values().filter(|row| row.is_state()) {
            if matches!(
                row.compare_observations(candidate_row),
                ObservationRowComparison::Equal | ObservationRowComparison::Undetermined
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
