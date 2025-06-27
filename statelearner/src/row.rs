use crate::observation::{
    Observation,
    ObservationComparison,
};
use crate::{
    ObservationValue,
    StateLearnerError,
    StateLearnerResult,
    SuffixId,
    SymbolId,
};
use slotmap::{
    SecondaryMap,
    SlotMap,
    new_key_type,
};
use std::fmt::{
    Debug,
    Display,
    Write,
};

new_key_type! { pub struct ObservationRowId; }

/// A row of observations for a state or candidate state.
/// By default, values in columns that have not been filled are defined as [Observation::Missing].
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObservationRow<T> {
    /// The observation row that is the predecessor of this row.
    /// This new row is arrived at by applying the transition symbol to the predecessor row.
    predecessor_row: Option<ObservationRowId>,
    /// The symbol that is applied to the predecessor row to arrive at this row.
    transition_symbol: Option<SymbolId>,
    /// The observations when applying the corresponding suffix to the
    /// target in the state represented by this row.
    observations: SecondaryMap<SuffixId, Observation<T>>,
    /// Is this row part of the upper or lower table?
    is_upper: bool,
}

pub enum ObservationRowComparison {
    Equal,
    /// The two observations are not equal.
    /// The provided suffix is the first witness of the inequality.
    NotEqual(SuffixId),
    Undetermined,
}

impl ObservationRowComparison {
    pub fn not_equal(&self) -> bool {
        matches!(self, ObservationRowComparison::NotEqual(_))
    }

    pub fn equal(&self) -> bool {
        matches!(self, ObservationRowComparison::Equal)
    }

    pub fn undetermined(&self) -> bool {
        matches!(self, ObservationRowComparison::Undetermined)
    }
}

impl<T> ObservationRow<T> {
    pub fn new_initial() -> Self {
        Self {
            predecessor_row: None,
            transition_symbol: None,
            observations: Default::default(),
            is_upper: true,
        }
    }

    pub fn new_candidate(
        predecessor: ObservationRowId,
        symbol: SymbolId,
    ) -> Self {
        Self {
            predecessor_row: Some(predecessor),
            transition_symbol: Some(symbol),
            observations: Default::default(),
            is_upper: false,
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

    pub fn observation_count(&self) -> usize {
        self.observations
            .values()
            .filter(|observation| !matches!(observation, Observation::Missing))
            .count()
    }

    pub fn predecessor_transition(&self) -> Option<(ObservationRowId, SymbolId)> {
        match (self.predecessor_row, self.transition_symbol) {
            (Some(row), Some(symbol)) => Some((row, symbol)),
            _ => None,
        }
    }

    #[inline]
    pub fn is_upper(&self) -> bool {
        self.is_upper
    }

    pub fn make_upper(&mut self) {
        self.is_upper = true;
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
        let mut result = if self.observations.len() != other.observations.len()
            || (self.observations.is_empty() && other.observations.is_empty())
        {
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
            match observation.compare(other_observation) {
                ObservationComparison::NotEqual => {
                    // If one of the observations is not equal, then the comparison is not equal.
                    // Even if some observations are still missing, we can already say for certain
                    // that the rows will never be equal.
                    return ObservationRowComparison::NotEqual(suffix);
                }
                ObservationComparison::Undetermined => {
                    // If one of the observations is undetermined, then the comparison might be
                    // undetermined. This can only be decided after we have looked at every
                    // observation.
                    result = ObservationRowComparison::Undetermined;
                }
                _ => {}
            };
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
    T: Display,
{
    pub fn fmt_debug_row(
        &self,
        self_name: &str,
        f: &mut impl Write,
        suffixes: &SlotMap<SuffixId, Vec<SymbolId>>,
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
                self.transition_symbol
                    .map_or("n/a".to_string(), |symbol| symbol.to_string()),
                self_name
            )
        )?;
        write!(f, "")?;
        for suffix_id in suffixes.keys() {
            if let Some(observation) = self.observations.get(suffix_id)
                && let Observation::Observed(observation) = observation
            {
                write!(f, "{:^20} ", format!("{}", observation))?;
            } else {
                write!(f, "{:^20} ", "n/a")?;
            }
        }
        Ok(())
    }
}
