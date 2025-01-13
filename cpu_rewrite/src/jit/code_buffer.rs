use crate::jit::api::{
    CustomOpArgs,
    JITResult,
    JitError,
};
use crate::jit::assembler::MogiAssembler;
use dynasmrt::AssemblyOffset;
use dynasmrt::relocations::Relocation;
use libc::{
    MADV_HUGEPAGE,
    MAP_ANONYMOUS,
    MAP_PRIVATE,
    PROT_EXEC,
    PROT_READ,
    PROT_WRITE,
};
use mogi_utilities::addresses::EVAddr;
use std::collections::HashMap;
use std::ptr::{
    NonNull,
    null_mut,
};
use thiserror::Error;

const GiB: usize = 1 << 30;

#[derive(Debug, Error)]
pub enum CodeBufferError {
    #[error("Failed to allocate memory for code buffer")]
    AllocationFailure,
}

/// This struct keeps track of the buffer accosiated with some executable code.
/// It consists of the pointer to the area of executable memory and its size.
#[derive(Debug)]
pub struct Code {
    ptr: NonNull<u8>,
    size: usize,
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
pub struct TargetBasicBlock<R: Relocation> {
    code: Code,
    /// Each basic block may call custom operations. The arguments are currently saved on the heap
    /// Once the basic block is destroyed, the call and its arguments are also removed.
    /// TODO: Check performance impact and maybe bake the arguments into the generated code?
    custom_op_args: Vec<Box<CustomOpArgs>>,
    assembler: MogiAssembler<R>,
    /// The assembly offset that points to the start of this block.
    start_offset: AssemblyOffset,
}

// TODO: In order to more easily create the basic blocks, it might make sense to
//       create a jumplist at the end of each basic block, where we can insert the
//       jumps to the following blocks

impl<R: Relocation + Clone> TargetBasicBlock<R> {
    /// Get the pointer to the start of executable code for this target block.
    pub fn ptr(&self) -> NonNull<u8> {
        self.code.ptr
    }

    pub fn assembler(&mut self) -> &mut MogiAssembler<R> {
        &mut self.assembler
    }

    pub fn start_offset(&self) -> AssemblyOffset {
        self.start_offset
    }

    pub fn add_custom_op_args(
        &mut self,
        args: Box<CustomOpArgs>,
    ) {
        self.custom_op_args.push(args);
    }

    /// Set the assembly offset for the code to execute when the block finishes.
    /// This will usually be the jit epilogue, but can also be a chained block.
    pub fn set_exit_success(
        &mut self,
        offset: AssemblyOffset,
    ) -> JITResult {
        self.assembler
            .labels_mut()
            .define_global("exit_success", offset)
            .map_err(|e| e.into())
    }

    /// Set the assembly offset for the code to execute when the jit runtime encounters an error.
    /// In case of a runtime error, the error field in the runtime context is set.
    pub fn set_exit_failure(
        &mut self,
        offset: AssemblyOffset,
    ) -> JITResult {
        self.assembler
            .labels_mut()
            .define_global("exit_failure", offset)
            .map_err(|e| e.into())
    }

    /// Commits pending changes to the assembly buffer and applies all relocation patches.
    /// Copies the bytes into the executable buffer.
    pub fn commit(&mut self) -> JITResult {
        let bytes = self.assembler.commit().map_err(JitError::AssemblerError)?;
        self.code.as_mut()[..bytes.len()].copy_from_slice(&bytes);
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
    blocks: HashMap<EVAddr, TargetBasicBlock<R>>,
    anonymous_blocks: Vec<TargetBasicBlock<R>>,
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
        estimated_size: usize,
    ) -> Result<(), CodeBufferError> {
        if estimated_size > self.size - self.occupied {
            todo!("Not enough space error")
        }
        let code = Code {
            ptr: unsafe { self.buf.add(self.occupied) },
            size: estimated_size,
        };

        self.blocks.insert(start, TargetBasicBlock {
            code,
            custom_op_args: vec![],
            assembler: unsafe { MogiAssembler::new(self.buf, self.occupied) },
            start_offset: AssemblyOffset(self.occupied),
        });
        self.occupied += estimated_size;
        Ok(())
    }

    pub fn block(
        &self,
        start: EVAddr,
    ) -> Option<&TargetBasicBlock<R>> {
        self.blocks.get(&start)
    }

    pub fn block_mut(
        &mut self,
        start: EVAddr,
    ) -> Option<&mut TargetBasicBlock<R>> {
        self.blocks.get_mut(&start)
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

        self.anonymous_blocks.push(TargetBasicBlock {
            code,
            custom_op_args: Vec::new(),
            assembler: unsafe { MogiAssembler::new(self.buf, self.occupied) },
            start_offset: AssemblyOffset(self.occupied),
        });
        self.occupied += size;
        Ok(AnonymousBlockId(self.anonymous_blocks.len() - 1))
    }

    pub fn anonymous_block(
        &self,
        id: AnonymousBlockId,
    ) -> &TargetBasicBlock<R> {
        &self.anonymous_blocks[id.0]
    }

    pub fn anonymous_block_mut(
        &mut self,
        id: AnonymousBlockId,
    ) -> &mut TargetBasicBlock<R> {
        &mut self.anonymous_blocks[id.0]
    }

    pub fn anonymous_block_offset(
        &self,
        id: AnonymousBlockId,
    ) -> AssemblyOffset {
        self.anonymous_blocks[id.0].start_offset()
    }
}
