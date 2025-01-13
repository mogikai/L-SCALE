use crate::jit::api::{
    JitError,
    PreludeSignature,
};
use crate::jit::code_buffer::{
    AnonymousBlockId,
    CodeBuffer,
};
use crate::jit::x86::assemble;
use dynasmrt::dynasm;
use dynasmrt::x64::X64Relocation;
use std::mem;

/// This function builds the jit prelude. It is a function that is called in order to enter the jited code.
/// The prelude is called like a C function, and thus only callee saved registers need to be preserved inside
/// the call. The Signature of the jit prelude function can be found here: [PreludeSignature]
pub fn build_prelude(
    code_buffer: &mut CodeBuffer<X64Relocation>,
    debug: bool,
) -> Result<(AnonymousBlockId, PreludeSignature), JitError> {
    let block_id = code_buffer.create_anonymous_block(1024)?;
    let block = code_buffer.anonymous_block_mut(block_id);
    if debug {
        assemble!(block.assembler();
            // For debugging we break on jumping into the prelude.
            ; int1
        );
    }
    assemble!(block.assembler();
        // See the System V ABI specification: https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf
        // Push callee saved registers onto stack
        ; push rbp
        ; push rbx
        ; push r12
        ; push r13
        ; push r14
        ; push r15
        // Move context to rbp. Function arguments are passed in RDI, RSI, RDX, RCX, R8, R9
        // TODO: We might need to save RSP somewhere if we want to modify it.
        //       Otherwise we will break the calling code, if we don't restore RSP correctly.
        ; mov rbp, rdi
        // Jump to provided block
        ; jmp rsi
    );
    block.commit()?;
    // TODO: It might be necessary to put additional info onto the stack or reserve some space for tmp values?
    Ok((block_id, unsafe { mem::transmute(block.ptr()) }))
}

/// Builds the epilogue of the jit. This block is jumped to, when the emulated code wants to exit into the emulator.
/// The code pops all the saved registers and then returns into the function that called the prelude.
pub fn build_epilogue(code_buffer: &mut CodeBuffer<X64Relocation>) -> Result<AnonymousBlockId, JitError> {
    let block_id = code_buffer.create_anonymous_block(1024).unwrap();
    let block = code_buffer.anonymous_block_mut(block_id);
    assemble!(block.assembler();
        ; pop r15
        ; pop r14
        ; pop r13
        ; pop r12
        ; pop rbx
        ; pop rbp
        ; ret
    );
    block.commit()?;
    Ok(block_id)
}
