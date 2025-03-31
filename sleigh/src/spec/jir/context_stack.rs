use crate::compiler::InternalCompilerResult;
use crate::lifter::context_provider::ContextProvider;
use crate::spec::moir::context::ContextId;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
    MOIRFieldParent,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecification,
    ProcessorSpecificationError,
};
use mogi_utilities::index_map::{
    MapKey,
    SecondaryMap,
};

#[derive(Debug, Default)]
pub(crate) struct ContextStack<'a> {
    // context ids are assumed to be contiguous and therefore pushed onto and popped
    // from this vector when saving and restoring state
    context_values: Vec<u128>,
    num_values: usize,
    context_fields: SecondaryMap<FieldId, Option<&'a MOIRField>>,
}

impl<'a> ContextStack<'a> {
    pub(crate) fn new<C: ContextProvider>(
        specification: &'a ProcessorSpecification,
        context_provider: &C,
    ) -> ProcessorSpecResult<Self> {
        let mut highest_cid = ContextId::try_from_index(0).expect("Zero is a valid index");
        let mut context_fields = SecondaryMap::default();

        for (field_id, field) in specification.symbol_tables.fields.items() {
            if let MOIRFieldParent::Context(cid) = field.parent {
                highest_cid = highest_cid.max(cid);
                context_fields[field_id] = Some(field);
            }
        }

        let mut slf = Self {
            // initialize default layer with zeroes
            context_values: vec![0; highest_cid.to_index() + 1],
            num_values: highest_cid.to_index() + 1,
            context_fields,
        };

        // fill initial values from the context provider
        for (field_id, value) in context_provider.values() {
            slf.set_field_value(field_id, value)?;
        }

        Ok(slf)
    }

    /// Get all stored `(FieldId, u128)` key-value pairs.
    pub(crate) fn values(&self) -> impl Iterator<Item = (FieldId, u128)> {
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
    ) -> ProcessorSpecResult<u128> {
        if cid.to_index() >= self.num_values {
            todo!("Generate error report for out of bounds context id");
        }

        let index = self.context_values.len() - cid.to_index() - 1;

        Ok(*self
            .context_values
            .get(index)
            .expect("We checked this above"))
    }

    /// Get a mutable reference to the value of a context register. Return an error if no value is
    /// associated with`cid`.
    pub(crate) fn context_value_mut(
        &mut self,
        cid: ContextId,
    ) -> ProcessorSpecResult<&mut u128> {
        if cid.to_index() >= self.num_values {
            todo!("Generate error report for out of bounds context id");
        }

        let index = self.context_values.len() - cid.to_index() - 1;

        Ok(self
            .context_values
            .get_mut(index)
            .expect("We checked this above"))
    }

    /// Get the value of context field `field_id`. Return an error if `field_id` does not point to a context field.
    pub(crate) fn field_value(
        &self,
        field_id: FieldId,
    ) -> ProcessorSpecResult<u128> {
        let field = self.context_fields[field_id].as_ref().expect("This must be set");
        let cid = match field.parent {
            MOIRFieldParent::Token(_) => panic!("We only store context fields"),
            MOIRFieldParent::Context(cid) => cid,
        };

        let context_value = self.context_value(cid)?;
        Ok(field.bits.extract(context_value))
    }

    /// Set the value of context field `field_id` to `new_value`. Return an error if `field_id` does not point to a context field.
    pub(crate) fn set_field_value(
        &mut self,
        field_id: FieldId,
        value: u128,
    ) -> ProcessorSpecResult {
        let field = self.context_fields[field_id]
            .as_ref()
            .ok_or_else(|| -> ProcessorSpecificationError {
                todo!("Generate error report for missing context field")
            })?;
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
