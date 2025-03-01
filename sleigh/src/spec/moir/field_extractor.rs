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

/// In most situations, fields can be extracted from the byte stream, but this doesn't hold for
/// fields that come after an unconstrained or partially constrained subtable. In this case field
/// extractors are stored in the jumplist itself. The byte index is relative to the absolute
/// beginning of the byte stream for known offsets.

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FieldExtractor {
    local_id: MOIRLocalId,
    local_size: MOIRSize,
    pub(crate) byte_index: usize,
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

    pub(crate) fn local_id(&self) -> MOIRLocalId {
        self.local_id
    }

    /// Extract and return the field value from the provided byte slice.
    pub(crate) fn extract_value(
        &self,
        bytes: &[u8],
    ) -> InternalCompilerResult<Option<JIRValue>> {
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

        trace_jir!("Extracted local_id {} := {}", self.local_id, extracted_value);

        Ok(Some(JIRValue::Const(JIRConst::new(extracted_value.into(), self.local_size))))
    }
}
