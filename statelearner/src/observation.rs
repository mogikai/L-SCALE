use crate::ObservationValue;

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Observation<T> {
    #[default]
    Missing,
    Observed(T),
}

pub enum ObservationComparison {
    Equal,
    NotEqual,
    Undetermined,
}

impl<T> Observation<T>
where
    T: ObservationValue,
{
    pub fn unwrap(&self) -> &T {
        match self {
            Observation::Missing => {
                panic!("unwrap on missing observation")
            }
            Observation::Observed(observation) => observation,
        }
    }

    pub fn compare(
        &self,
        other: &Self,
    ) -> ObservationComparison {
        // If all observations are equal, then the comparison returns equal.
        // If one or more differences are found, then the comparison is not equal.
        // If one or both observations are missing, then the comparison is undetermined.
        match (self, other) {
            (Observation::Observed(a), Observation::Observed(b)) if a == b => ObservationComparison::Equal,
            (Observation::Observed(a), Observation::Observed(b)) if a != b => ObservationComparison::NotEqual,
            _ => ObservationComparison::Undetermined,
        }
    }
}
