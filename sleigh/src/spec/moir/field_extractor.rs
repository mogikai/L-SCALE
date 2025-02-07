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
    byte_index: usize,
    bits: MOIRBitRange,
}

impl FieldExtractor {
    pub(crate) fn new(
        local_id: MOIRLocalId,
        local_size: MOIRSize,
        byte_index: usize,
        bits: MOIRBitRange,
    ) -> Self {
        Self {
            local_id,
            local_size,
            byte_index,
            bits,
        }
    }

    /// Extract the field value from the provided byte slice.
    pub(crate) fn process_value(
        &self,
        bytes: &[u8],
        values: &mut SecondaryMap<MOIRLocalId, Option<JIRValue>>,
    ) -> InternalCompilerResult {
        if bytes.len() <= self.byte_index + usize::from(self.bits.bits().checked_sub(1).unwrap_or(0) / 8) {
            todo!("Generate error report for out of bounds token index");
        }

        let mut value_bytes = [0u8; 8];
        for (bytei, byte) in bytes[self.byte_index..]
            .iter()
            .enumerate()
            .take(self.bits.end.div_ceil(8).into())
        {
            value_bytes[bytei] = *byte;
        }

        let value = u64::from_le_bytes(value_bytes);
        let extracted_value = self.bits.extract(value);

        trace_jir!("Defining local_id {} := {}", self.local_id, extracted_value);

        values[self.local_id] = Some(JIRValue::Const(JIRConst::new(extracted_value.into(), self.local_size)));

        Ok(())
    }
}
