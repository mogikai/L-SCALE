use crate::jit::api::{
    CustomOpArgs,
    JITResult,
    JitError,
};
use crate::jit::assembler::MogiAssembler;
use dynasmrt::relocations::Relocation;
use dynasmrt::{
    DynamicLabel,
    DynasmLabelApi,
};
use libc::{
    MADV_HUGEPAGE,
    MAP_ANONYMOUS,
    MAP_PRIVATE,
    PROT_EXEC,
    PROT_READ,
    PROT_WRITE,
};
use mogi_sleigh::spec::jir::jir_block::JIRBlockId;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use std::collections::HashMap;
use std::ptr::{
    NonNull,
    null_mut,
};
use thiserror::Error;

#[allow(non_upper_case_globals)]
const GiB: usize = 1 << 30;

#[derive(Debug, Error)]
pub enum CodeBufferError {
    #[error("Failed to allocate memory for code buffer")]
    AllocationFailure,
}

/// This struct keeps track of the buffer accosiated with some executable code.
/// It consists of the pointer to the area of executable memory and its size.
#[derive(Debug, Copy, Clone)]
pub struct Code {
    ptr: NonNull<u8>,
    size: usize,
}

impl Code {
    pub fn ptr(&self) -> NonNull<u8> {
        self.ptr
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

impl AsRef<[u8]> for Code {
    fn as_ref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.size) }
    }
}

impl AsMut<[u8]> for Code {
    fn as_mut(&mut self) -> &mut [u8] {
        // SAFETY: Transforming the underlying code into a slice is safe, since
        //         the creator of the Code struct must assert that the pointer and size
        //         point to a valid contiguous region of memory.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.size) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnonymousBlockId(usize);

/// A compiled target basic block. Points to a region inside the executable buffer.
/// The compiled block also contains a list of arguments for any custom op calls.
#[derive(Debug)]
pub struct CompiledBasicBlock<R: Relocation> {
    /// The code location where this basic block is encoded.
    /// If this is a newly created block, there might not yet be a code location allocated to this block.
    code: Option<Code>,
    /// Each basic block may call custom operations. The arguments are currently saved on the heap
    /// Once the basic block is destroyed, the call and its arguments are also removed.
    /// TODO: Check performance impact and maybe bake the arguments into the generated code?
    custom_op_args: Vec<Box<CustomOpArgs>>,
    assembler: MogiAssembler<R>,
    labels: SecondaryMap<JIRBlockId, Option<DynamicLabel>>,
    /// The next block that needs to be executed when following the normal execution path.
    /// This is cleared when the block ends with a non-conditional goto instruction.
    fallthrough: Option<EVAddr>,
}

// TODO: In order to more easily create the basic blocks, it might make sense to
//       create a jumplist at the end of each basic block, where we can insert the
//       jumps to the following blocks

impl<R: Relocation + Clone> CompiledBasicBlock<R> {
    /// Get the pointer to the start of executable code for this target block.
    pub fn ptr(&self) -> Option<NonNull<u8>> {
        self.code.map(|c| c.ptr)
    }

    pub fn fallthrough(&self) -> Option<EVAddr> {
        self.fallthrough
    }

    pub(super) fn clear_fallthrough(&mut self) {
        self.fallthrough = None;
    }

    /// Create or get the dynamic label associated with the provided block id
    pub fn dynamic_label(
        &mut self,
        block_id: JIRBlockId,
    ) -> DynamicLabel {
        let label = &mut self.labels[block_id];
        *label.get_or_insert_with(|| self.assembler.new_dynamic_label())
    }

    pub fn assembler(&mut self) -> &mut MogiAssembler<R> {
        &mut self.assembler
    }

    pub fn add_custom_op_args(
        &mut self,
        args: Box<CustomOpArgs>,
    ) {
        self.custom_op_args.push(args);
    }

    /// Set the target block for the code to execute when the block finishes.
    /// This will usually be the jit epilogue, but can also be a chained block.
    /// # Safety
    /// TODO
    pub unsafe fn set_exit_success(
        &mut self,
        target: NonNull<u8>,
    ) {
        unsafe { self.assembler.set_global_label("exit_success", target) }
    }

    pub unsafe fn set_exit_alternate(
        &mut self,
        target: NonNull<u8>,
    ) {
        unsafe { self.assembler.set_global_label("exit_alternate", target) }
    }

    /// Set the target block for the code to execute when the jit runtime encounters an error.
    /// In case of a runtime error, the error field in the runtime context is set.
    /// # Safety
    /// TODO
    pub unsafe fn set_exit_failure(
        &mut self,
        target: NonNull<u8>,
    ) {
        unsafe { self.assembler.set_global_label("exit_failure", target) }
    }

    pub fn encoded_size(&self) -> usize {
        self.assembler.encoded_size()
    }

    pub fn needs_relocation(&self) -> bool {
        let Some(code) = self.code else { return false };
        self.assembler.encoded_size() > code.size
    }

    pub fn relocate(
        &mut self,
        _new_code_location: Code,
    ) -> JITResult {
        todo!()
    }

    /// Commits pending changes to the assembly buffer and applies all relocation patches.
    /// Copies the bytes into the executable buffer.
    pub fn commit(&mut self) -> JITResult {
        let Some(code) = self.code else { todo!("Correct error") };
        // Write the assembly buffer to executable code and perform relocations.
        // TODO: Describe why this is safe to call
        unsafe {
            self.assembler
                .write(code)
                .map_err(JitError::AssemblerError)?;
        }
        Ok(())
    }
}

pub struct CodeBuffer<R: Relocation> {
    buf: NonNull<u8>,
    size: usize,
    /// The area of the code buffer that is already occupied.
    /// TODO: keep a list of free areas.
    occupied: usize,
    /// Keeps track of the blocks we have already compiled.
    blocks: HashMap<EVAddr, CompiledBasicBlock<R>>,
    anonymous_blocks: Vec<CompiledBasicBlock<R>>,
}

impl<R: Relocation + Clone> CodeBuffer<R> {
    pub fn new() -> Result<Self, CodeBufferError> {
        let size = 1 * GiB;
        // This is safe, since we return an error on failure.
        let buf = unsafe {
            let buf =
                libc::mmap(null_mut(), size, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
            if libc::madvise(buf, size, MADV_HUGEPAGE) != 0 {
                Err(CodeBufferError::AllocationFailure)?
            }
            NonNull::new(buf as *mut u8).ok_or(CodeBufferError::AllocationFailure)?
        };
        Ok(Self {
            buf,
            size,
            occupied: 0,
            blocks: Default::default(),
            anonymous_blocks: Default::default(),
        })
    }

    pub fn create_block(
        &mut self,
        start: EVAddr,
        successor: Option<EVAddr>,
    ) -> Result<(), CodeBufferError> {
        self.blocks.insert(
            start,
            CompiledBasicBlock {
                code: None,
                custom_op_args: vec![],
                assembler: MogiAssembler::new(),
                labels: Default::default(),
                fallthrough: successor,
            },
        );
        Ok(())
    }

    pub fn block(
        &self,
        start: EVAddr,
    ) -> Option<&CompiledBasicBlock<R>> {
        self.blocks.get(&start)
    }

    pub fn block_mut(
        &mut self,
        start: EVAddr,
    ) -> Option<&mut CompiledBasicBlock<R>> {
        self.blocks.get_mut(&start)
    }

    /// Writes the blocks assembly to the code buffer, potentially allocating a new area for the assembly to reside in.
    pub fn write_block(
        &mut self,
        evaddr: EVAddr,
    ) -> JITResult {
        let block = self
            .blocks
            .get_mut(&evaddr)
            .ok_or_else(|| -> JitError { todo!("Return correct error") })?;
        let required_size = block.encoded_size();
        if required_size > self.size - self.occupied {
            todo!("Emit error for insufficient size")
        }
        block.code = Some(Code {
            ptr: unsafe { self.buf.byte_add(self.occupied) },
            size: required_size,
        });
        self.occupied += required_size;
        block.commit()?;
        Ok(())
    }

    /// Create a new anonymous block (i.e. it has no EVAddr) and return its id.
    pub fn create_anonymous_block(
        &mut self,
        size: usize,
    ) -> Result<AnonymousBlockId, CodeBufferError> {
        if size > self.size - self.occupied {
            todo!("Not enough space error")
        }
        let code = Code {
            // SAFETY: self.occupied is guaranteed to be <= self.size.
            //         The memory in question spans from self.buf to self.buf + self.size
            ptr: unsafe { self.buf.add(self.occupied) },
            size,
        };

        self.anonymous_blocks.push(CompiledBasicBlock {
            code: Some(code),
            custom_op_args: Vec::new(),
            assembler: MogiAssembler::new(),
            labels: Default::default(),
            fallthrough: None,
        });
        self.occupied += size;
        Ok(AnonymousBlockId(self.anonymous_blocks.len() - 1))
    }

    pub fn anonymous_block(
        &self,
        id: AnonymousBlockId,
    ) -> &CompiledBasicBlock<R> {
        &self.anonymous_blocks[id.0]
    }

    pub fn anonymous_block_mut(
        &mut self,
        id: AnonymousBlockId,
    ) -> &mut CompiledBasicBlock<R> {
        &mut self.anonymous_blocks[id.0]
    }

    pub fn flush(&mut self) {
        self.blocks.clear();
        self.occupied = unsafe {
            self.anonymous_blocks
                .iter()
                .map(|block| {
                    block
                        .code
                        .map_or(self.buf, |code| code.ptr().offset(code.size() as isize))
                })
                .max()
                .unwrap_or(self.buf)
                .byte_offset_from_unsigned(self.buf)
        };
    }
}
