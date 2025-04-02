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

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub(crate) struct FieldExtractor {
    local_id: MOIRLocalId,
    local_size: MOIRSize,
    byte_index: usize,
    bits: MOIRBitRange,
    signed: bool,
}

impl FieldExtractor {
    pub(crate) fn new(
        local_id: MOIRLocalId,
        local_size: MOIRSize,
        byte_index: usize,
        bits: MOIRBitRange,
        signed: bool,
    ) -> Self {
        Self {
            local_id,
            local_size,
            byte_index,
            bits,
            signed,
        }
    }

    pub(crate) fn local_id(&self) -> MOIRLocalId {
        self.local_id
    }

    pub(crate) fn byte_index(&self) -> usize {
        self.byte_index
    }

    /// Extract and return the field value from the provided byte slice.
    pub(crate) fn extract_value(
        &self,
        bytes: &[u8],
    ) -> InternalCompilerResult<Option<JIRValue>> {
        if bytes.len() <= self.byte_index + usize::from(self.bits.bits().checked_sub(1).unwrap_or(0) / 8) {
            todo!("Generate error report for out of bounds token index");
        }

        let mut value_bytes = [0u8; 16];
        for (bytei, byte) in bytes[self.byte_index..]
            .iter()
            .enumerate()
            .take(self.bits.end.div_ceil(8).into())
        {
            value_bytes[bytei] = *byte;
        }

        let value = u128::from_le_bytes(value_bytes);
        let mut value = self.bits.extract_u128(value);

        if self.signed {
            // if the field is signed, we need to sign extend
            value = (((value << (128 - self.bits.bits())) as i128) >> (128 - self.bits.bits())) as u128;
        };

        trace_jir!("Extracted local_id {} := {}", self.local_id, value);

        Ok(Some(JIRValue::Const(JIRConst::new(
            value,
            self.local_size
                .try_into()
                .expect("The size must fit into JIRSize"),
        ))))
    }
}
