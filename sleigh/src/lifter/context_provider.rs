use std::collections::HashMap;

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

#[derive(Default)]
pub struct SimpleContextProvider {
    values: HashMap<FieldId, u64>
}

impl ContextProvider for SimpleContextProvider {
    fn set(
        &mut self,
        processor_spec: &ProcessorSpecification,
        name: &str,
        value: u64,
    ) {
        processor_spec.field_id_by_string(name).map(|field_id| {
            self.values.insert(field_id, value);
        });
    }

    fn get_by_string(
        &self,
        processor_spec: &ProcessorSpecification,
        name: &str,
    ) -> Option<u64> {
        processor_spec
            .field_id_by_string(name)
            .and_then(|field_id| self.values.get(&field_id).copied())
    }

    fn get_by_id(
        &self,
        field_id: FieldId,
    ) -> Option<u64> {
        self.values.get(&field_id).copied()
    }

    fn values(&self) -> impl Iterator<Item = (FieldId, u64)> {
        self.values
            .iter()
            .map(|(field_id, value)| (*field_id, *value))
    }
}
