use chumsky::container::Container;
use imbl::HashMap;

use crate::compiler::SleighCompilerResult;
use crate::compiler::error::SleighCompilerError;
use crate::spec::moir::context::MOIRContextId;
use crate::spec::moir::field::{
    FieldId,
    MOIRBitRange,
    MOIRField,
    MOIRFieldParent,
};
use crate::spec::typed_index_table::TypedIndexTable;

#[derive(Debug, Default)]
pub(crate) struct ContextProvider<'a> {
    pub(crate) context_values: HashMap<MOIRContextId, u64>,
    context_fields: HashMap<FieldId, &'a MOIRField>,

    // TODO improve
    states: Vec<HashMap<MOIRContextId, u64>>,
}

impl<'a> ContextProvider<'a> {
    pub(crate) fn new(fields: &'a TypedIndexTable<FieldId, MOIRField>) -> Self {
        let mut context_values = HashMap::new();
        let mut context_fields = HashMap::new();

        for (field_id, field) in fields.iter() {
            if let MOIRFieldParent::Context(cid) = field.parent {
                context_values.insert(cid, 0);
                context_fields.insert(field_id, field);
            }
        }

        Self {
            context_values,
            context_fields,
            states: vec![],
        }
    }

    /// Get the number of context fields stored in this context provider.
    pub(crate) fn len(&self) -> usize {
        self.context_fields.len()
    }

    /// Get all stored `(FieldId, u64)` key-value pairs.
    pub(crate) fn values(&self) -> impl Iterator<Item = (FieldId, u64)> {
        self.context_fields.iter().map(|(field_id, _)| {
            (
                *field_id,
                self.field_value(*field_id)
                    .expect("If we store the field we store the context"),
            )
        })
    }

    /// Get the value of a context register. Return an error if no value is associated with`cid`.
    pub(crate) fn context_value(
        &self,
        cid: MOIRContextId,
    ) -> SleighCompilerResult<u64> {
        Ok(*self
            .context_values
            .get(&cid)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing context value") })?)
    }

    /// Get a mutable reference to the value of a context register. Return an error if no value is
    /// associated with`cid`.
    pub(crate) fn context_value_mut(
        &mut self,
        cid: MOIRContextId,
    ) -> SleighCompilerResult<&mut u64> {
        self.context_values
            .get_mut(&cid)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing context value") })
    }

    /// Get the value of context field `field_id`. Return an error if `field_id` does not point to a context field.
    pub(crate) fn field_value(
        &self,
        field_id: FieldId,
    ) -> SleighCompilerResult<u64> {
        let field = self
            .context_fields
            .get(&field_id)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing context field") })?;
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
        value: u64,
    ) -> SleighCompilerResult {
        let field = self
            .context_fields
            .get(&field_id)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing context field") })?;
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
        self.states.push(self.context_values.clone());
    }

    /// The context will be set for a successful match on the current layer, but needs to be reset
    /// in case we get stuck in a dead end when descending into subtables. This function
    /// undoes the latest pushed changes.
    pub(crate) fn pop_state(&mut self) -> SleighCompilerResult {
        self.context_values = self
            .states
            .pop()
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for popping an empty state") })?;
        Ok(())
    }
}
