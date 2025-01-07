use imbl::HashMap;

use crate::{compiler::{error::SleighCompilerError, SleighCompilerResult}, spec::{moir::{context::MOIRContextId, field::{FieldId, MOIRBitRange, MOIRField, MOIRFieldParent}}, typed_index_table::TypedIndexTable}};

#[derive(Debug, Default)]
pub(crate) struct ContextProvider<'a> {
    field_values: HashMap<FieldId, u64>,
    context_values: HashMap<MOIRContextId, u64>,
    context_fields: HashMap<FieldId, &'a MOIRField>,
}

impl<'a> ContextProvider<'a> {
    pub(crate) fn new(fields: &'a TypedIndexTable<FieldId, MOIRField>) -> Self {
        let mut field_values = HashMap::new();
        let mut context_values = HashMap::new();
        let mut context_fields = HashMap::new();

        for (field_id, field) in fields.iter() {
            if let MOIRFieldParent::Context(cid) = field.parent {
                field_values.insert(field_id, 0);
                context_values.insert(cid, 0);
            }

            context_fields.insert(field_id, field);
        }

        Self { field_values, context_values, context_fields }
    }

    /// Get all stored `(FieldId, u64)` key-value pairs.
    pub(crate) fn values(&self) -> impl Iterator<(FieldId, u64)> {
        self.context_fields.iter().map(|(field_id, field)| {
            let cid = match field.parent {
                MOIRFieldParent::Token(token_id) => panic!("This should not happen"),
                MOIRFieldParent::Context(cid) => cid,
            };

            let parent_value = *self.values.get(&cid).expect("There must be a value");
            let field_value = field.bits.extract(parent_value);
            (*field_id, field_value)
        })
    }

    /// Get the number of context fields stored in this context provider.
    pub(crate) fn len(&self) -> usize {
        self.context_fields.len()
    }

    /// Get the value of context field `field_id`. Returns an error if `field_id` does not point to a context field.
    pub(crate) fn context_field(&self, field_id: FieldId) -> SleighCompilerResult<u64> {
        let bits = self.context_fields.get(&field_id).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing context field") })?.bits;
        let value = self.values.get(&field_id).expect("If the field is a context field it must have a value");
        Ok(bits.extract(value))
    }

    /// Set the value of context field `field_id` to `value`. Returns an error if `field_id` does not point to a context field.
    pub(crate) fn set_context_field(&mut self, field_id: FieldId, new_value: u64) -> SleighCompilerResult {
        let bits = self.context_fields.get(&field_id).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing context field") })?.bits.clone();
        let value = self.values.get_mut(&field_id).expect("If the field is a context field it must have a value");
        let cleared = value & !bits.mask_shift();
        *value = cleared | bits.insert(new_value);
        Ok(())
    }
}
