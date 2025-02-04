use mogi_utilities::index_map::SecondaryMap;
use serde_derive::{
    Deserialize,
    Serialize,
};

use crate::compiler::InternalCompilerResult;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRConst,
    JIRValue,
};

use super::common::MOIRSize;
use super::field::MOIRBitRange;
use super::scope::MOIRLocalId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FieldExtractor {
    local_id: MOIRLocalId,
    local_size: MOIRSize,
    token_index: usize,
    bits: MOIRBitRange,
}

impl FieldExtractor {
    pub(crate) fn new(
        local_id: MOIRLocalId,
        local_size: MOIRSize,
        token_index: usize,
        bits: MOIRBitRange,
    ) -> Self {
        Self {
            local_id,
            local_size,
            token_index,
            bits,
        }
    }

    /// Extract parts of the field from `token_value` and insert into `fields`. Might need to be
    /// called multiple times since fields that cross byte boundaries can be fragmented across
    /// multiple jumplists.
    pub(crate) fn process_value(
        &self,
        tokens: &Vec<u64>,
        values: &mut SecondaryMap<MOIRLocalId, Option<JIRValue>>,
    ) -> InternalCompilerResult {
        if tokens.len() <= self.token_index {
            todo!("Generate error report for out of bounds token index");
        }

        trace_jir!("Tokens: {:?}", tokens);
        trace_jir!("Extracting field from token {:?} with bits {:?}", tokens[self.token_index], self.bits);
        let value = self.bits.extract(tokens[self.token_index]);
        trace_jir!("Defining local_id {} := {}", self.local_id, value);

        values[self.local_id] = Some(JIRValue::Const(JIRConst::new(value.into(), self.local_size)));

        Ok(())
    }
}
