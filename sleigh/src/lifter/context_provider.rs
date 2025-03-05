use crate::spec::ProcessorSpecification;
use crate::spec::moir::field::FieldId;

pub trait ContextProvider {
    /// Set the provided context register field `name` to `value`.
    /// The processor specification is needed to resolve strings to internal ids.
    fn set(
        &mut self,
        processor_spec: &ProcessorSpecification,
        name: &str,
        value: u64,
    );

    fn get_by_string(
        &self,
        processor_spec: &ProcessorSpecification,
        name: &str,
    ) -> Option<u64>;

    fn get_by_id(
        &self,
        field_id: FieldId,
    ) -> Option<u64>;

    /// Return an iterator over all stored context field values.
    fn values(&self) -> impl Iterator<Item = (FieldId, u64)>;
}
