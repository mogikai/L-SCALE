use crate::compiler::InternalCompilerResult;
use crate::compiler::error::InternalError;
use crate::spec::ProcessorSpecification;
use crate::spec::moir::context::ContextId;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
    MOIRFieldParent,
};
use mogi_utilities::index_map::SecondaryMap;
use num::PrimInt;

#[derive(Debug, Default)]
pub struct ContextStack {
    // context ids are assumed to be contiguous and therefore pushed onto and popped
    // from this vector when saving and restoring state
    context_values: Vec<u64>,
    num_values: usize,
    context_fields: SecondaryMap<FieldId, Option<MOIRField>>,
}

impl ContextStack {
    pub(crate) fn new(specification: &ProcessorSpecification) -> Self {
        let mut highest_cid = ContextId::from(0);
        let mut context_fields = SecondaryMap::default();

        for (field_id, field) in specification.symbol_tables.fields.items() {
            if let MOIRFieldParent::Context(cid) = field.parent {
                highest_cid = highest_cid.max(cid);
                context_fields[field_id] = Some(field.clone());
            }
        }

        Self {
            // initialize default layer with zeroes
            context_values: vec![0; usize::from(highest_cid) + 1],
            num_values: usize::from(highest_cid) + 1,
            context_fields,
        }
    }

    /// Get all stored `(FieldId, u64)` key-value pairs.
    pub(crate) fn values(&self) -> impl Iterator<Item = (FieldId, u64)> {
        self.context_fields.items().filter_map(|(field_id, field)| {
            field.as_ref().and_then(|_| {
                Some((
                    field_id,
                    self.field_value(field_id)
                        .expect("If we store the field we store the context"),
                ))
            })
        })
    }

    /// Get the value of a context register. Return an error if no value is associated with`cid`.
    pub(crate) fn context_value(
        &self,
        cid: ContextId,
    ) -> InternalCompilerResult<u64> {
        if usize::from(cid) >= self.num_values {
            todo!("Generate error report for out of bounds context id");
        }

        let index = self.context_values.len() - usize::from(cid) - 1;

        Ok(*self
            .context_values
            .get(index)
            .expect("We checked this above"))
    }

    /// Get a mutable reference to the value of a context register. Return an error if no value is
    /// associated with`cid`.
    pub fn context_value_mut(
        &mut self,
        cid: ContextId,
    ) -> InternalCompilerResult<&mut u64> {
        if usize::from(cid) >= self.num_values {
            todo!("Generate error report for out of bounds context id");
        }

        let index = self.context_values.len() - usize::from(cid) - 1;

        Ok(self
            .context_values
            .get_mut(index)
            .expect("We checked this above"))
    }

    /// Get the value of context field `field_id`. Return an error if `field_id` does not point to a context field.
    pub(crate) fn field_value(
        &self,
        field_id: FieldId,
    ) -> InternalCompilerResult<u64> {
        let field = self.context_fields[field_id]
            .as_ref()
            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing context field") })?;
        let cid = match field.parent {
            MOIRFieldParent::Token(_) => panic!("We only store context fields"),
            MOIRFieldParent::Context(cid) => cid,
        };

        let context_value = self.context_value(cid)?;
        Ok(field.bits.extract(context_value).reverse_bits())
    }

    /// Set the value of context field `field_id` to `new_value`. Return an error if `field_id` does not point to a context field.
    pub(crate) fn set_field_value(
        &mut self,
        field_id: FieldId,
        value: u64,
    ) -> InternalCompilerResult {
        let field = self.context_fields[field_id]
            .as_ref()
            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing context field") })?;
        let cid = match field.parent {
            MOIRFieldParent::Token(_) => panic!("We only store context fields"),
            MOIRFieldParent::Context(cid) => cid,
        };

        let bits = field.bits;

        let context_value = self.context_value_mut(cid)?;
        let cleared = *context_value & !bits.shifted_mask();
        *context_value = cleared | bits.insert(value);
        Ok(())
    }

    /// The context will be set for a successful match on the current layer, but needs to be reset
    /// in case we get stuck in a dead end when descending into subtables. This function
    /// stores the current state so changes can be undone.
    pub(crate) fn push_state(&mut self) {
        let start = self.context_values.len() - self.num_values;

        for i in 0..self.num_values {
            self.context_values.push(self.context_values[start + i]);
        }
    }

    /// The context will be set for a successful match on the current layer, but needs to be reset
    /// in case we get stuck in a dead end when descending into subtables. This function
    /// undoes the latest pushed changes.
    pub(crate) fn pop_state(&mut self) -> InternalCompilerResult {
        // the default layer can't be popped
        if self.context_values.len() < 2 * self.num_values {
            todo!("Generate error report for popping empty state");
        }

        self.context_values
            .truncate(self.context_values.len() - self.num_values);
        Ok(())
    }
}
