use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::trace_jit;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::x64emitter::helpers::reg;
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
        source: impl Into<JIRValue>,
    ) {
        requirement.add_request(OperandRequest::int_write_any(target));
        if let JIRValue::Slice(source) = source.into() {
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

        let target_reg = reg!(self.allocations, inst, 0);
        match source {
            JIRValue::Slice(source) => {
                let source = reg!(self.allocations, inst, 1);
                self.copy_reg(target_reg, source, size, CopyExtendMode::None)
            }
            JIRValue::Const(source) => self.load_const(target_reg, source),
        }
    }

    /// Emit a copy instruction that copies `source` to `target`.
    /// In case a copy instruction would be a no-op, e.g. if source == target, then
    /// nothing will be emitted.
    #[inline]
    pub fn copy_reg(
        &mut self,
        target: HostReg,
        source: HostReg,
        size: JIRSize,
        extend_mode: CopyExtendMode,
    ) -> JITResult {
        match target.class() {
            RegClass::Int => self.copy_reg_int(target, source, size, extend_mode),
            RegClass::Vector => self.copy_reg_vec(target, source, size),
            RegClass::Float => todo!("copy_reg for floats"),
        }
    }

    fn copy_reg_int(
        &mut self,
        target: HostReg,
        source: HostReg,
        size: JIRSize,
        extend_mode: CopyExtendMode,
    ) -> JITResult {
        match extend_mode {
            CopyExtendMode::ZeroExtend(new_size) => {
                trace_jit!("Zero extending from {source:?}:{size} to {target:?}:{new_size}");
                if target == source && size == new_size {
                    trace_jit!("Zero extend with equal sizes: no-op");
                    return Ok(());
                }
                self.copy_reg_zext(target, source, size, new_size)?;
                Ok(())
            }
            CopyExtendMode::SignExtend(new_size) => {
                trace_jit!("Sign extending from {source:?}:{size} to {target:?}:{new_size}");
                if target == source && size == new_size {
                    trace_jit!("Sign extend with equal sizes: no-op");
                    return Ok(());
                }
                self.copy_reg_sext(target, source, size, new_size)?;
                Ok(())
            }
            CopyExtendMode::None => {
                trace_jit!("Copying {source:?} to {target:?} ({size})");
                // Only emit a copy instruction, if target and source are different.
                if target == source {
                    return Ok(());
                }
                match size {
                    1..=4 => {
                        assemble!(self.block.assembler();
                            ; mov Rd(target.hw_enc()), Rd(source.hw_enc())
                        );
                    }
                    5..=8 => {
                        assemble!(self.block.assembler();
                            ; mov Rq(target.hw_enc()), Rq(source.hw_enc())
                        );
                    }
                    _ => unimplemented!("Unsupported size for gpr copy"),
                }
                Ok(())
            }
        }
    }

    fn copy_reg_zext(
        &mut self,
        target: HostReg,
        source: HostReg,
        source_size: JIRSize,
        target_size: JIRSize,
    ) -> JITResult {
        match target_size {
            1 => match source_size {
                1 => assemble!(self.block.assembler();
                    ; mov Rb(target.hw_enc()), Rb(source.hw_enc())
                ),
                2.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            2 => match source_size {
                1 => assemble!(self.block.assembler();
                    ; movzx Rw(target.hw_enc()), Rb(source.hw_enc())
                ),
                2 => assemble!(self.block.assembler();
                    ; mov Rw(target.hw_enc()), Rw(source.hw_enc())
                ),
                3.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            4 => match source_size {
                1 => assemble!(self.block.assembler();
                    ; movzx Rd(target.hw_enc()), Rb(source.hw_enc())
                ),
                2 => assemble!(self.block.assembler();
                    ; movzx Rd(target.hw_enc()), Rw(source.hw_enc())
                ),
                4 => assemble!(self.block.assembler();
                    ; mov Rd(target.hw_enc()), Rd(source.hw_enc())
                ),
                5.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            8 => match source_size {
                // moving to the lower 32 bit automatically zeros the upper 32 bit.
                1 => assemble!(self.block.assembler();
                    ; movzx Rd(target.hw_enc()), Rb(source.hw_enc())
                ),
                2 => assemble!(self.block.assembler();
                    ; movzx Rd(target.hw_enc()), Rw(source.hw_enc())
                ),
                4 => assemble!(self.block.assembler();
                    ; mov Rd(target.hw_enc()), Rd(source.hw_enc())
                ),
                8 => assemble!(self.block.assembler();
                    ; mov Rq(target.hw_enc()), Rq(source.hw_enc())
                ),
                9.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            _ => todo!("Unsupported zero extend target"),
        }
        Ok(())
    }

    fn copy_reg_sext(
        &mut self,
        target: HostReg,
        source: HostReg,
        source_size: JIRSize,
        target_size: JIRSize,
    ) -> JITResult {
        match target_size {
            1 => match source_size {
                1 => assemble!(self.block.assembler();
                    ; mov Rb(target.hw_enc()), Rb(source.hw_enc())
                ),
                2.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            2 => match source_size {
                1 => assemble!(self.block.assembler();
                    ; movsx Rw(target.hw_enc()), Rb(source.hw_enc())
                ),
                2 => assemble!(self.block.assembler();
                    ; mov Rw(target.hw_enc()), Rw(source.hw_enc())
                ),
                3.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            4 => match source_size {
                1 => assemble!(self.block.assembler();
                    ; movsx Rd(target.hw_enc()), Rb(source.hw_enc())
                ),
                2 => assemble!(self.block.assembler();
                    ; movsx Rd(target.hw_enc()), Rw(source.hw_enc())
                ),
                4 => assemble!(self.block.assembler();
                    ; mov Rd(target.hw_enc()), Rd(source.hw_enc())
                ),
                5.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            8 => match source_size {
                // moving to the lower 32 bit automatically zeros the upper 32 bit.
                1 => assemble!(self.block.assembler();
                    ; movsx Rq(target.hw_enc()), Rb(source.hw_enc())
                ),
                2 => assemble!(self.block.assembler();
                    ; movsx Rq(target.hw_enc()), Rw(source.hw_enc())
                ),
                4 => assemble!(self.block.assembler();
                    ; movsx Rq(target.hw_enc()), Rd(source.hw_enc())
                ),
                8 => assemble!(self.block.assembler();
                    ; mov Rq(target.hw_enc()), Rq(source.hw_enc())
                ),
                9.. => todo!("Narrowing zero extend makes no sense"),
                _ => todo!("Unsupported zero extend size"),
            },
            _ => todo!("Unsupported zero extend target"),
        }
        Ok(())
    }

    fn copy_reg_vec(
        &mut self,
        target: HostReg,
        source: HostReg,
        size: JIRSize,
    ) -> JITResult {
        match size {
            4 => {
                // single-precision
                assemble!(self.block.assembler();
                    ; movss Rx(target.hw_enc()), Rx(source.hw_enc())
                );
            }
            8 => {
                // double-precision
                assemble!(self.block.assembler();
                    ; movsd Rx(target.hw_enc()), Rx(source.hw_enc())
                );
            }
            size => {
                todo!("Unsupported size {size}")
            }
        }

        Ok(())
    }

    /// Writes a constant directly into the memory location associated with the target slice.
    /// This function may use the scratch register.
    #[inline]
    pub(super) fn write_const_to_mem(
        &mut self,
        target: JIRSlice,
        value: JIRConst,
    ) -> JITResult {
        debug_assert_eq!(target.size(), value.size());
        let value = value.value();
        let slice_offset = i32::try_from(X86TaskContext::slice_offset(target))
            .map_err(|_| -> JitError { todo!("Invalid offset size") })?;
        match target.size() {
            1 => {
                // Move to 32 bit register to zero extend.
                assemble!(self.block.assembler();
                    ; gs mov BYTE [slice_offset], BYTE value as _
                );
            }
            2 => {
                // Move to 32 bit register to zero extend.
                assemble!(self.block.assembler();
                    ; gs mov WORD [slice_offset], WORD value as _
                );
            }
            4 => {
                // We need to move to 32 bit register here, since the 64 bit move sign extends?????
                assemble!(self.block.assembler();
                    ; gs mov DWORD [slice_offset], DWORD value as _
                );
            }
            8 => {
                // We don't care which RegClass this is and just use int since only the size is relevant.
                let scratch = self.allocations.scratch_register(RegClass::Int);
                // A 64-bit immediate can only be moved into registers.
                assemble!(self.block.assembler();
                    ; mov Rq(scratch.hw_enc()), QWORD value as _
                    ; gs mov QWORD [slice_offset], Rq(scratch.hw_enc())
                );
            }
            size => {
                todo!("Unsupported size {size}")
            }
        }
        Ok(())
    }

    /// Writes a register directly into the memory location associated with the target slice.
    /// This function may use the scratch register.
    #[inline]
    pub(super) fn write_int_reg_to_mem(
        &mut self,
        target: JIRSlice,
        reg: HostReg,
    ) -> JITResult {
        let slice_offset = i32::try_from(X86TaskContext::slice_offset(target))
            .map_err(|_| -> JitError { todo!("Invalid offset size") })?;
        let reg = reg.hw_enc();
        match target.size() {
            1 => {
                assemble!(self.block.assembler();
                    ; gs mov BYTE [slice_offset as _], Rb(reg)
                );
            }
            2 => {
                assemble!(self.block.assembler();
                    ; gs mov WORD [slice_offset as _], Rw(reg)
                );
            }
            4 => {
                assemble!(self.block.assembler();
                    ; gs mov DWORD [slice_offset as _], Rd(reg)
                );
            }
            8 => {
                assemble!(self.block.assembler();
                    ; gs mov QWORD [slice_offset as _], Rq(reg)
                );
            }
            _ => {
                todo!("Unsupported store size")
            }
        }
        Ok(())
    }

    /// Emit an instruction that loads the provided constant into the provided target gp register.
    #[inline]
    pub(super) fn load_const(
        &mut self,
        target: HostReg,
        value: JIRConst,
    ) -> JITResult {
        trace_jit!("Loading {value:?} into {target:?}");
        match target.class() {
            RegClass::Int => self.load_const_int(target, value)?,
            RegClass::Vector => self.load_const_vec(target, value)?,
            RegClass::Float => todo!("Implement float const load"),
        }

        Ok(())
    }

    fn load_const_int(
        &mut self,
        target: HostReg,
        value: JIRConst,
    ) -> JITResult {
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

    fn load_const_vec(
        &mut self,
        target: HostReg,
        value: JIRConst,
    ) -> JITResult {
        let size = value.size();
        let value = value.value();

        let scratch = self.allocations.scratch_register(RegClass::Int);
        match size {
            ..=8 => {
                // things that fit into a quadword register can be moved directly
                assemble!(self.block.assembler();
                    ; mov Rq(scratch.hw_enc()), QWORD value as _
                    ; movq Rx(target.hw_enc()), Rq(scratch.hw_enc())
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
}

#[derive(Debug)]
pub enum CopyExtendMode {
    /// Extend to the provided size
    ZeroExtend(JIRSize),
    /// Sign extend to the provided size.
    SignExtend(JIRSize),
    None,
}
