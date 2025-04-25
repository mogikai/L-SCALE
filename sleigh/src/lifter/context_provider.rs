use std::collections::HashMap;

use crate::spec::moir::field::FieldId;
use crate::spec::{
    CustomOpId,
    ProcessorSpecification,
};

pub trait ContextProvider {
    /// Set the provided context register field `name` to `value`.
    /// The processor specification is needed to resolve strings to internal ids.
    fn set(
        &mut self,
        processor_spec: &ProcessorSpecification,
        name: &str,
        value: u128,
    );

    fn get_by_string(
        &self,
        processor_spec: &ProcessorSpecification,
        name: &str,
    ) -> Option<u128>;

    fn get_by_id(
        &self,
        field_id: FieldId,
    ) -> Option<u128>;

    /// Return an iterator over all stored context field values.
    fn values(&self) -> impl Iterator<Item = (FieldId, u128)>;
}

#[derive(Default)]
pub struct SimpleContextProvider {
    values: HashMap<FieldId, u128>,
}

impl ContextProvider for SimpleContextProvider {
    fn set(
        &mut self,
        processor_spec: &ProcessorSpecification,
        name: &str,
        value: u128,
    ) {
        processor_spec.field_id_by_string(name).map(|field_id| {
            self.values.insert(field_id, value);
        });
    }

    fn get_by_string(
        &self,
        processor_spec: &ProcessorSpecification,
        name: &str,
    ) -> Option<u128> {
        processor_spec
            .field_id_by_string(name)
            .and_then(|field_id| self.values.get(&field_id).copied())
    }

    fn get_by_id(
        &self,
        field_id: FieldId,
    ) -> Option<u128> {
        self.values.get(&field_id).copied()
    }

    fn values(&self) -> impl Iterator<Item = (FieldId, u128)> {
        self.values
            .iter()
            .map(|(field_id, value)| (*field_id, *value))
    }
}
