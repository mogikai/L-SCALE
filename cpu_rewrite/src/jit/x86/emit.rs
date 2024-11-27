//! This module contains all the functions that are used to emit code for [JIRInstruction]s

use crate::jit::api::JitError;
use crate::jit::x86::{
    assemble,
    X86MogiJit,
};
use dynasmrt::dynasm;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

mod arithmetic;

/// Emit a 64 bit mov instruction, but only if the registers differ.
macro_rules! mov64 {
    ($asm:expr; $dest_reg:expr, $src_reg:expr) => {
        use dynasmrt::DynasmApi;
        use dynasmrt::dynasm;
        if $dest_reg != $src_reg {
            assemble!($asm;
                ; mov Rq($dest_reg), Rq($src_reg)
            );
        }
    };
}

use mov64;

impl X86MogiJit {
    pub fn emit_copy(
        &mut self,
        target: JIRSlice,
        source: JIRValue,
    ) -> Result<(), JitError> {
        debug_assert_eq!(target.size(), source.size());

        let size = target.size();

        // Allocate and associate a GP register with the slice.
        // No load is needed, since we overwrite the data anyway for a copy.
        let target_reg = self.get_slice_gp(target) as u8;
        match source {
            JIRValue::Slice(slice) => {
                if size <= 8 {
                    let reg = self.load_slice_gp(slice) as u8;
                    let block = self.block_mut()?;
                    assemble!(block.assembler;
                        ; mov Rq(target_reg), Rq(reg)
                    );
                } else {
                    todo!("Support sizes > 8")
                }
            }
            JIRValue::Const(value) => {
                let value = value.value();
                let block = self.block_mut()?;
                match size {
                    1 => {
                        assemble!(block.assembler;
                            ; mov Rb(target_reg), BYTE value as _
                        );
                    }
                    2 => {
                        assemble!(block.assembler;
                            ; mov Rw(target_reg), WORD value as _
                        );
                    }
                    3..=4 => {
                        assemble!(block.assembler;
                            ; mov Rq(target_reg), DWORD value as _
                        );
                    }
                    5..=8 => {
                        assemble!(block.assembler;
                            ; mov Rq(target_reg), QWORD value as _
                        );
                    }
                    size => {
                        todo!("Unsupported size {size}")
                    }
                }
            }
        }
        Ok(())
    }
}
