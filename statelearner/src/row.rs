use crate::ovservation::{
    Observation,
    ObservationComparison,
};
use crate::{
    ObservationValue,
    StateLearnerError,
    StateLearnerResult,
    SuffixId,
    WordId,
};
use slotmap::{
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use std::fmt::{
    Debug,
    Write,
};

new_key_type! { pub struct ObservationRowId; }

/// A row of observations for a state or candidate state.
/// By default, values in columns that have not been filled are defined as [Observation::Missing].
#[derive(Debug)]
pub struct ObservationRow<T> {
    /// The observation row that is the predecessor of this row.
    /// This new row is arrived at by applying the transition word to the predecessor row.
    predecessor_row: Option<ObservationRowId>,
    /// The word that is applied to the predecessor row to arrive at this row.
    transition_word: Option<WordId>,
    /// The observations when applying the corresponding suffix to the
    /// target in the state represented by this row.
    observations: SecondaryMap<SuffixId, Observation<T>>,
    /// Is this a candidate or a state?
    is_state: bool,
}

pub enum ObservationRowComparison {
    Equal,
    /// The two observations are not equal.
    /// The provided suffix is the first witness of the inequality.
    NotEqual(SuffixId),
    Undetermined,
}

impl<T> ObservationRow<T> {
    pub fn new_initial() -> Self {
        Self {
            predecessor_row: None,
            transition_word: None,
            observations: Default::default(),
            is_state: true,
        }
    }

    pub fn new_candidate(
        predecessor: ObservationRowId,
        word: WordId,
    ) -> Self {
        Self {
            predecessor_row: Some(predecessor),
            transition_word: Some(word),
            observations: Default::default(),
            is_state: false,
        }
    }

    pub fn observation(
        &self,
        suffix: SuffixId,
    ) -> &Observation<T> {
        self.observations
            .get(suffix)
            .unwrap_or(&Observation::Missing)
    }

    pub fn predecessor_transition(&self) -> Option<(ObservationRowId, WordId)> {
        match (self.predecessor_row, self.transition_word) {
            (Some(row), Some(word)) => Some((row, word)),
            _ => None,
        }
    }

    #[inline]
    pub fn is_state(&self) -> bool {
        self.is_state
    }

    pub fn make_state(&mut self) {
        self.is_state = true;
    }
}

impl<T> ObservationRow<T>
where
    T: ObservationValue,
{
    pub fn compare_observations(
        &self,
        other: &Self,
    ) -> ObservationRowComparison {
        let result = if self.observations.len() != other.observations.len() {
            // If the lengths are not equal, then the comparison is undetermined, since one observation
            // row is missing observations.
            // We cannot shortcut here, since it might still be possible that the rows can be determined
            // to not be equal.
            ObservationRowComparison::Undetermined
        } else {
            // Otherwise we set the result to equal for now.
            ObservationRowComparison::Equal
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
                return ObservationRowComparison::NotEqual(suffix);
            }
        }

        // If we did not determine the rows to be not equal, then the result must be undetermined or equal.
        result
    }

    pub fn set_observation(
        &mut self,
        suffix: SuffixId,
        observation: T,
    ) -> StateLearnerResult {
        if let Some(old_observation) = self.observations.get(suffix) {
            if let Observation::Observed(old_observation) = old_observation {
                if old_observation != &observation {
                    Err(StateLearnerError::ConflictingObservation)?
                }
            }
        }
        self.observations
            .insert(suffix, Observation::Observed(observation));
        Ok(())
    }
}

impl<T> ObservationRow<T>
where
    T: Debug,
{
    pub fn fmt_debug_row(
        &self,
        self_name: &str,
        f: &mut impl Write,
        suffixes: &SlotMap<SuffixId, Vec<WordId>>,
    ) -> std::fmt::Result {
        let pred_str = self
            .predecessor_row
            .map(|pred| format!("{}", pred))
            .unwrap_or("n/a".into());
        write!(
            f,
            "{:<40}",
            format!(
                "({:<10} --[{:^12}]--> {:>10}) ",
                pred_str,
                self.transition_word
                    .map_or("n/a".to_string(), |word| word.to_string()),
                self_name
            )
        )?;
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
