//! This module contains all the functions that are used to emit code for [JIRInstruction]s

use crate::jit::api::JitError;
use crate::jit::x86::{
    assemble,
    GPRegister,
    X86MogiJit,
};
use dynasmrt::dynasm;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

mod arithmetic;
mod bitwise;
mod logic;

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
    /// Emit instructions to copy the source [JIRValue] into the target [JIRSlice].
    pub fn emit_copy(
        &mut self,
        target: JIRSlice,
        source: JIRValue,
    ) -> Result<(), JitError> {
        debug_assert_eq!(target.size(), source.size());

        let size = target.size();

        // Allocate and associate a GP register with the slice.
        // No load is needed, since we overwrite the data anyway for a copy.
        let target_reg = self.get_slice_gp(target);
        match source {
            JIRValue::Slice(slice) => {
                self.emit_load_slice_to_gpr(target_reg, slice)?;
            }
            JIRValue::Const(value) => {
                self.emit_load_const_to_gpr(target_reg, value)?;
            }
        }
        Ok(())
    }

    /// Load the [JIRSlice] `slice` into the [GPRegister] `target_reg`.
    /// If the slice is already loaded in `target_reg` this emits nothing.
    fn emit_load_slice_to_gpr(
        &mut self,
        target_reg: GPRegister,
        slice: JIRSlice,
    ) -> Result<(), JitError> {
        let size = slice.size();
        if size <= 8 {
            // TODO: It might make sense to be able to hint the gpr to load to for load_slice_gp.
            //       We could omit a few mov instructions in some cases that way.
            //       I.e. if the slice is not loaded yet, use the provided hint, and otherwise
            //       we just return the gpr to which the slice was already loaded.
            let reg = self.load_slice_gp(slice);
            let block = self.block_mut()?;
            // Only emit a copy instruction, if the slice was loaded into a different register.
            if reg != target_reg {
                assemble!(block.assembler;
                    ; mov Rq(target_reg as u8), Rq(reg as u8)
                );
            }
        } else {
            todo!("Support sizes > 8")
        }
        Ok(())
    }

    /// Emit an instruction that loads the provided constant into the provided target gp register.
    fn emit_load_const_to_gpr(
        &mut self,
        target_reg: GPRegister,
        value: JIRConst,
    ) -> Result<(), JitError> {
        let size = value.size();
        let value = value.value();
        let block = self.block_mut()?;
        match size {
            1 => {
                assemble!(block.assembler;
                    ; mov Rb(target_reg as u8), BYTE value as _
                );
            }
            2 => {
                assemble!(block.assembler;
                    ; mov Rw(target_reg as u8), WORD value as _
                );
            }
            3..=4 => {
                assemble!(block.assembler;
                    ; mov Rq(target_reg as u8), DWORD value as _
                );
            }
            5..=8 => {
                assemble!(block.assembler;
                    ; mov Rq(target_reg as u8), QWORD value as _
                );
            }
            size => {
                todo!("Unsupported size {size}")
            }
        }
        Ok(())
    }
}
