use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn copy_requirements(
        requirement: &mut AllocRequirement,
        target: JIRSlice,
        source: JIRValue,
    ) {
        requirement.add_request(OperandRequest::int_write_any(target));
        if let JIRValue::Slice(source) = source {
            requirement.add_request(OperandRequest::int_read_any(source));
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions to copy the source [JIRValue] into the target [JIRSlice].
    #[inline]
    pub fn emit_copy(
        &mut self,
        inst: InstId,
        target: JIRSlice,
        source: impl Into<JIRValue>,
    ) -> JITResult {
        let source = source.into();
        let size = target.size();
        debug_assert_eq!(size, source.size());

        if target == source {
            // Nothing to do here
            return Ok(());
        }

        // Allocate and associate a GP register with the slice.
        // No load is needed, since we overwrite the data anyway for a copy.
        let Allocation::Register(target_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        match source {
            JIRValue::Slice(source) => {
                let Allocation::Register(source) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };
                self.copy_reg(target_reg, source, size, CopyExtendMode::None)
            }
            JIRValue::Const(source) => self.load_const(target_reg, source),
        }
    }

    #[inline]
    pub fn copy_reg(
        &mut self,
        target: HostReg,
        source: HostReg,
        size: JIRSize,
        extend_mode: CopyExtendMode,
    ) -> JITResult {
        if !matches!(target.class(), RegClass::Int) {
            todo!("Implement non int copy")
        }
        debug_assert_eq!(target.class(), source.class());
        let target = target.hw_enc();
        let source = source.hw_enc();
        match extend_mode {
            CopyExtendMode::ZeroExtend(new_size) => {
                todo!()
                // if target == source && size == new_size {
                //     trace_jit!("Zero extend with equal sizes: no-op");
                //     return Ok(());
                // }
                // trace_jit!("Zero extending from {source:?}:{size} to {target:?}:{new_size}");
                // self.copy_gpr_to_gpr_zext(source, target, size, new_size)?;
                // Ok(())
            }
            CopyExtendMode::SignExtend(new_size) => {
                todo!()
                // if target == source && size == new_size {
                //     trace_jit!("Sign extend with equal sizes: no-op");
                //     return Ok(());
                // }
                // trace_jit!("Sign extending from {source:?}:{size} to {target:?}:{new_size}");
                // self.copy_gpr_to_gpr_sext(source, target, size, new_size)?;
                // Ok(())
            }
            CopyExtendMode::None => {
                // Only emit a copy instruction, if target and source are different.
                if target == source {
                    return Ok(());
                }
                match size {
                    1..=4 => {
                        assemble!(self.block.assembler();
                            ; mov Rd(target), Rd(source)
                        );
                    }
                    5..=8 => {
                        assemble!(self.block.assembler();
                            ; mov Rq(target), Rq(source)
                        );
                    }
                    _ => unimplemented!("Unsupported size for gpr copy"),
                }
                Ok(())
            }
        }
    }

    /// Emit an instruction that loads the provided constant into the provided target gp register.
    #[inline]
    pub(super) fn load_const(
        &mut self,
        target: HostReg,
        value: JIRConst,
    ) -> JITResult {
        if !matches!(target.class(), RegClass::Int) {
            todo!("Implement non int const load")
        }
        let size = value.size();
        let value = value.value();
        if value == 0 {
            // Zeroing out a register can be done more efficiently
            return self.load_zero(target, size);
        }
        let target = target.hw_enc();
        match size {
            1 => {
                // Move to 32 bit register to zero extend.
                assemble!(self.block.assembler();
                    ; mov Rd(target), BYTE value as _
                );
            }
            2 => {
                // Move to 32 bit register to zero extend.
                assemble!(self.block.assembler();
                    ; mov Rd(target), WORD value as _
                );
            }
            3..=4 => {
                // We need to move to 32 bit register here, since the 64 bit move sign extends?????
                assemble!(self.block.assembler();
                    ; mov Rd(target), DWORD value as _
                );
            }
            5..=8 => {
                assemble!(self.block.assembler();
                    ; mov Rq(target), QWORD value as _
                );
            }
            size => {
                todo!("Unsupported size {size}")
            }
        }
        Ok(())
    }

    pub fn load_zero(
        &mut self,
        target_reg: HostReg,
        size: JIRSize,
    ) -> JITResult {
        match size {
            1..=8 => {
                // For now we just zero the entire register.
                // in the future it might make sense to do more specific stuff here
                // For now, it is enough to do xor r32, r32.
                // This also zeros the upper 32 bits for 64 bit registers (see intel sdm)
                assemble!(self.block.assembler();
                    ; xor Rd(target_reg.hw_enc()), Rd(target_reg.hw_enc())
                );
            }
            _ => {
                panic!("Unsupported size")
            }
        }
        Ok(())
    }

    /// Emit instructions to copy the source [JIRValue] into the target [JIRSlice].
    #[inline]
    pub fn emit_copy_deprecated(
        &mut self,
        target: JIRSlice,
        source: impl Into<JIRValue>,
    ) -> JITResult {
        let source = source.into();
        debug_assert_eq!(target.size(), source.size());

        let size = target.size();

        if target == source {
            // If target==source we just need to make sure the target is loaded.
            self.load_slice_gpr(target, SliceType::Output);
            return Ok(());
        }

        // Allocate and associate a GP register with the slice.
        // No load is needed, since we overwrite the data anyway for a copy.
        let target_reg = self.allocate_slice_gpr(target, SliceType::Output);
        self.emit_load_value_to_gpr(target_reg, source)
    }
}
