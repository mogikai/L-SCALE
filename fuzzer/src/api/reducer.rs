use crate::api::error::FuzzerError;

pub trait Reducer {
    type Output;

    /// Reduces the observations of the associated observers or reducers to the specified output type.
    /// In most cases this will probably be a boolean decision of whether or not an observation is
    /// considered interesting.
    fn reduce(&mut self) -> Result<Self::Output, FuzzerError>;
}
