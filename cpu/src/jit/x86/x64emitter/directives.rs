use crate::jit::api::JITResult;
use crate::jit::assembler::MogiAssembler;
use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use crate::jit::regalloc::result::LoadSpillDirective;
use crate::jit::trace_jit;
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::x64emitter::X64Emitter;
use dynasmrt::relocations::Relocation;
use dynasmrt::x64::X64Relocation;
use mogi_sleigh::spec::jir::values::JIRSlice;
use tracing::trace;

impl X64Emitter<'_> {
    pub fn emit_directives(
        &mut self,
        directives: &[LoadSpillDirective],
    ) -> JITResult {
        for directive in directives {
            trace_jit!("Emitting directive: {directive:?}");
            match directive {
                LoadSpillDirective::Load(reg, slice) => {
                    self.loaded_slices.insert(*reg, *slice);
                    emit_load_directive(self.block.assembler(), *reg, *slice)?
                }
                LoadSpillDirective::Spill(reg, slice) => {
                    self.loaded_slices.remove(*reg);
                    emit_spill_directive(self.block.assembler(), *reg, *slice)?
                }
                LoadSpillDirective::Move(_, _) => todo!(),
            }
        }
        Ok(())
    }

    /// Reloads all loaded slices from the cpu context.
    /// In almost all cases, this will only make sense
    /// if the slices were saved beforehand.
    pub fn reload_loaded_slices(&mut self) -> JITResult {
        trace_jit!("Emitting code to reload loaded slices: {:?}", self.loaded_slices);
        for (reg, slice) in self.loaded_slices.iter() {
            emit_load_directive(self.block.assembler(), reg, *slice)?;
        }
        Ok(())
    }

    /// Preserves all loaded slices in the cpu context.
    pub fn save_loaded_slices(&mut self) -> JITResult {
        trace_jit!("Emitting code to save loaded slices: {:?}", self.loaded_slices);
        for (reg, slice) in self.loaded_slices.iter() {
            emit_spill_directive(self.block.assembler(), reg, *slice)?;
        }
        Ok(())
    }
}

fn emit_load_directive<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    slice: JIRSlice,
) -> JITResult
where
    R: Relocation,
{
    match reg.class() {
        RegClass::Int => load_slice_to_int_reg(assembler, reg, slice),
        RegClass::Float => {}
        RegClass::Vector => {}
    }
    Ok(())
}

#[inline]
fn load_slice_to_int_reg<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    slice: JIRSlice,
) where
    R: Relocation,
{
    // We want to load the provided slice into the register.
    // We first need to calculate the offset in the cpu context.
    let slice_offset = X86TaskContext::slice_offset(slice);
    // Emit the instruction that loads the slice from the specified space in the context.
    // We always load to a 32 bit register. This avoids false dependencies.
    let reg = reg.hw_enc();
    match slice.size() {
        1 => {
            assemble!(assembler;
                ; movzx Rd(reg), BYTE [cpu_context + slice_offset as _]
            );
        }
        2 => {
            assemble!(assembler;
                ; movzx Rd(reg), WORD [cpu_context + slice_offset as _]
            );
        }
        4 => {
            // Moving to the lower 32 bits of a register zeros the upper 32 bits
            assemble!(assembler;
                ; mov Rd(reg), DWORD [cpu_context + slice_offset as _]
            );
        }
        8 => {
            assemble!(assembler;
                ; mov Rq(reg), QWORD [cpu_context + slice_offset as _]
            );
        }
        _ => {
            panic!("Unsupported load size")
        }
    }
}

fn emit_spill_directive<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    slice: JIRSlice,
) -> JITResult
where
    R: Relocation,
{
    match reg.class() {
        RegClass::Int => spill_int_reg_to_slice(assembler, reg, slice),
        RegClass::Float => todo!(),
        RegClass::Vector => todo!(),
    }
}

fn spill_int_reg_to_slice<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    slice: JIRSlice,
) -> JITResult
where
    R: Relocation,
{
    debug_assert_eq!(reg.class(), RegClass::Int);
    let varnode_offset = X86TaskContext::slice_offset(slice);
    // Emit the instruction that loads the varnode from the specified space in the context.
    let reg = reg.hw_enc();
    match slice.size() {
        1 => {
            assemble!(assembler;
                ; mov BYTE [cpu_context + varnode_offset as _], Rb(reg as u8)
            );
        }
        2 => {
            assemble!(assembler;
                ; mov WORD [cpu_context + varnode_offset as _], Rw(reg as u8)
            );
        }
        4 => {
            assemble!(assembler;
                ; mov DWORD [cpu_context + varnode_offset as _], Rd(reg as u8)
            );
        }
        8 => {
            assemble!(assembler;
                ; mov QWORD [cpu_context + varnode_offset as _], Rq(reg as u8)
            );
        }
        _ => {
            panic!("Unsupported store size")
        }
    }
    Ok(())
}
