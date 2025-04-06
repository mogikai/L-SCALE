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
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
use dynasmrt::relocations::Relocation;
use dynasmrt::x64::X64Relocation;
use mogi_sleigh::spec::jir::common::JIRSize;
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
                LoadSpillDirective::Move { dst, src, size } => {
                    self.loaded_slices.r#move(*dst, *src);
                    self.copy_reg(*dst, *src, *size, CopyExtendMode::None)?
                }
                LoadSpillDirective::Adjust {
                    reg,
                    old_slice,
                    new_slice,
                } => {
                    // Overwrite the old loaded slice info with the adjusted one.
                    self.loaded_slices.insert(*reg, *new_slice);
                    emit_adjust_directive(
                        self.block.assembler(),
                        *reg,
                        self.allocations.scratch_register(RegClass::Int),
                        *old_slice,
                        *new_slice,
                    )?
                }
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
        RegClass::Float => todo!(),
        RegClass::Vector => todo!(),
    }
}

#[inline]
fn load_slice_to_int_reg<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    slice: JIRSlice,
) -> JITResult
where
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
    Ok(())
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
                ; mov BYTE [cpu_context + varnode_offset as _], Rb(reg)
            );
        }
        2 => {
            assemble!(assembler;
                ; mov WORD [cpu_context + varnode_offset as _], Rw(reg)
            );
        }
        4 => {
            assemble!(assembler;
                ; mov DWORD [cpu_context + varnode_offset as _], Rd(reg)
            );
        }
        8 => {
            assemble!(assembler;
                ; mov QWORD [cpu_context + varnode_offset as _], Rq(reg)
            );
        }
        _ => {
            panic!("Unsupported store size")
        }
    }
    Ok(())
}

fn emit_adjust_directive<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    scratch: HostReg,
    old_slice: JIRSlice,
    new_slice: JIRSlice,
) -> JITResult
where
    R: Relocation,
{
    match reg.class() {
        RegClass::Int => adjust_int(assembler, reg, scratch, old_slice, new_slice),
        RegClass::Float => todo!(),
        RegClass::Vector => todo!(),
    }
}

fn adjust_int<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    scratch: HostReg,
    old_slice: JIRSlice,
    new_slice: JIRSlice,
) -> JITResult
where
    R: Relocation,
{
    assert_eq!(reg.class(), RegClass::Int);
    if old_slice.offset == new_slice.offset {
        adjust_int_same_offset(assembler, reg, scratch, old_slice, new_slice)
    } else {
        adjust_int_different_offset(assembler, reg, scratch, old_slice, new_slice)
    }
}

fn adjust_int_same_offset<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    scratch: HostReg,
    old_slice: JIRSlice,
    new_slice: JIRSlice,
) -> JITResult
where
    R: Relocation,
{
    assert_eq!(old_slice.offset, new_slice.offset);
    match (old_slice.size(), new_slice.size()) {
        (old_size, new_size) if old_size < new_size => {
            if new_size > 8 {
                todo!("Unsupported size")
            }
            // The already loaded slice is smaller, we can just load the additional contents.
            let to_load = new_size - old_size;
            // We load the currently unloaded data into the scratch register
            let unloaded = new_slice.slice(old_size.into(), to_load);
            let old_data_mask = (1 << old_size) - 1;
            load_slice_to_int_reg(assembler, scratch, unloaded)?;
            assemble!(assembler;
                // Make sure that the registers upper bits are all zero and do not contain any trash.
                ; and Rq(scratch.hw_enc()), old_data_mask as _
                // Shift the loaded data
                ; shl Rq(scratch.hw_enc()), old_size as _
                // Or it into the old data.
                ; or Rq(reg.hw_enc()), Rq(scratch.hw_enc())
            );
            Ok(())
        }
        (old_size, new_size) if old_size > new_size => {
            if old_size > 8 {
                todo!("Unsupported size")
            }
            // The already loaded slice is larger. We flush the content that is no longer needed.
            let to_store = old_size - new_size;
            let to_unload = old_slice.slice(old_size.into(), to_store);
            assemble!(assembler;
                // Move the whole data to scratch
                ; mov Rq(scratch.hw_enc()), Rq(reg.hw_enc())
                // Shift right so we only have the part that needs a flush
                ; shr Rq(scratch.hw_enc()), to_store as _
            );
            // Spill the data that is no longer needed.
            spill_int_reg_to_slice(assembler, scratch, to_unload)
        }
        (old, new) if old == new => Ok((/* Nothing to do here */)),
        _ => unreachable!(),
    }
}

fn adjust_int_different_offset<R>(
    assembler: &mut MogiAssembler<R>,
    reg: HostReg,
    scratch: HostReg,
    old_slice: JIRSlice,
    new_slice: JIRSlice,
) -> JITResult
where
    R: Relocation,
{
    // TODO: Do this more efficiently?
    spill_int_reg_to_slice(assembler, reg, old_slice)?;
    load_slice_to_int_reg(assembler, reg, new_slice)
}
