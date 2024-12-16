use crate::lifter::cache::LifterCache;
use crate::spec::ProcessorSpecification;
use crate::spec::builder::CustomInterner;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::local_resolver::JIRResolver;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
    VarNodeId,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::table::TableId;
use mogi_utilities::addresses::EVAddr;
use source::TargetInstructionSource;
use thiserror::Error;
use tracing::debug;

pub mod cache;
pub mod source;

pub type LifterResult<T = ()> = Result<T, LifterError>;

#[derive(Debug, Error, Copy, Clone)]
pub enum LifterError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The address {0:?} is already associated with a cached instruction")]
    InstructionAlreadyCached(EVAddr),
    #[error("The address {0:?} is already associated with a cached block")]
    BlockAlreadyCached(EVAddr),
    #[error("The cache has no entry for the requested address {0:?}")]
    MissingCacheEntry(EVAddr),
}

pub struct Lifter<'a, 'cache, InstSourceT: TargetInstructionSource> {
    specification: &'a ProcessorSpecification,
    cache: &'cache mut LifterCache,
    instruction_source: &'a InstSourceT,
}

impl<'a, 'cache: 'a, InstSourceT: TargetInstructionSource> Lifter<'a, 'cache, InstSourceT> {
    pub fn new(
        specification: &'a ProcessorSpecification,
        cache: &'cache mut LifterCache,
        instruction_source: &'a InstSourceT,
    ) -> Self {
        Self {
            specification,
            cache,
            instruction_source,
        }
    }
    /// Lift a single instruction if possible, by fetching the bytes for the instruction from
    /// `source`. Returns the address of the next instruction, and the lifted instruction
    /// One target instruction corresponds to multiple lifted instructions most of the time.
    /// See [TargetInstruction] for more detail.
    pub fn lift<'slf>(
        &mut self,
        addr: EVAddr,
        cid: usize,
    ) -> LifterResult<(EVAddr, &TargetInstruction)> {
        self.cache.flush_instruction(addr)?;
        // Check the cache. If it contains the instruction we can simply return it.
        if self.cache.is_instruction_cached(addr) {
            // TODO: Remove the outer check once the conditional control flow borrow checking lands
            //       See https://github.com/rust-lang/rfcs/blob/master/text/2094-nll.md#problem-case-3-conditional-control-flow-across-functions
            match self.cache.fetch_instruction(addr) {
                Ok(instruction) => {
                    return Ok((instruction.next(), instruction));
                }
                Err(_) => unreachable!(),
            }
        }
        // No cache entry? Fetch data now.
        let _data = self.instruction_source.load(addr);
        // TODO: Perform the matching on the instruction bytes to
        //       get the correct constructor and resolve context
        let constructor = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(cid))
            .expect("");
        debug!("{}", constructor.mcode_debug_string(&self.specification.interner));
        // TODO: Calculate size of instruction and add it here.
        // TODO: For inst_next2 we need to decode the next instruction as well.
        //       since this is expensive, we probably should buffer this, and
        //       also only calculate inst_next2 if we actually need it in the body.
        //       For now we just set it to None
        let inst_next = addr + 8;
        let mut resolver = JIRResolver::new(&self.specification, constructor, addr, inst_next, None);
        resolver
            .define_ident(
                self.specification.interner.lookup("vexVVVV_r32").unwrap(),
                JIRValue::Slice(JIRSlice::new(VarNodeId::from(0), SpaceId::from(3), 0, 4)),
            )
            .expect("");
        resolver
            .define_ident(
                self.specification.interner.lookup("rm32").unwrap(),
                JIRValue::Slice(JIRSlice::new(VarNodeId::from(0), SpaceId::from(3), 0, 4)),
            )
            .expect("");

        let constructor3 = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(5))
            .expect("");
        let mut resolver3 = JIRResolver::new(&self.specification, constructor3, addr, inst_next, None);
        resolver3
            .define_ident(
                self.specification.interner.lookup("reg32").unwrap(),
                JIRValue::Slice(JIRSlice::new(VarNodeId::from(5), SpaceId::from(3), 0, 4)),
            )
            .expect("");
        resolver
            .define_subtable_resolver(TableId::from(3), resolver3)
            .expect("");

        let constructor43 = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(172))
            .expect("");
        let resolver43 = JIRResolver::new(&self.specification, constructor43, addr, inst_next, None);

        let constructor9 = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(20))
            .expect("");
        let mut resolver9 = JIRResolver::new(&self.specification, constructor9, addr, inst_next, None);
        resolver9
            .define_ident(
                self.specification.interner.lookup("r32").unwrap(),
                JIRValue::Slice(JIRSlice::new(VarNodeId::from(5), SpaceId::from(3), 0, 4)),
            )
            .expect("");

        let constructor39 = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(130))
            .expect("");
        let mut resolver39 = JIRResolver::new(&self.specification, constructor39, addr, inst_next, None);
        resolver39
            .define_subtable_resolver(TableId::from(9), resolver9)
            .expect("");

        let constructor46 = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(200))
            .expect("");
        let mut resolver46 = JIRResolver::new(&self.specification, constructor46, addr, inst_next, None);
        resolver46
            .define_subtable_resolver(TableId::from(43), resolver43)
            .expect("");
        resolver46
            .define_subtable_resolver(TableId::from(39), resolver39)
            .expect("");

        let constructor118 = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(358))
            .expect("");
        let mut resolver118 = JIRResolver::new(&self.specification, constructor118, addr, inst_next, None);
        resolver118
            .define_subtable_resolver(TableId::from(46), resolver46)
            .expect("");
        resolver
            .define_subtable_resolver(TableId::from(118), resolver118)
            .expect("");

        let sub_constructor = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(366))
            .expect("");
        let mut sub_resolver = JIRResolver::new(&self.specification, sub_constructor, addr, inst_next, None);
        sub_resolver
            .define_ident(
                self.specification.interner.lookup("reg32").unwrap(),
                JIRValue::Slice(JIRSlice::new(VarNodeId::from(0), SpaceId::from(3), 0, 4)),
            )
            .expect("");
        sub_resolver
            .define_ident(
                self.specification.interner.lookup("reg64").unwrap(),
                JIRValue::Slice(JIRSlice::new(VarNodeId::from(0), SpaceId::from(3), 0, 8)),
            )
            .expect("");
        resolver
            .define_subtable_resolver(TableId::from(123), sub_resolver)
            .expect("");

        // Generate the jcode for the instruction.
        let instruction = constructor.generate_jcode(resolver).expect("");
        // Now we can cache it for future access.
        let instruction = self.cache.cache_instruction(addr, instruction)?;
        Ok((instruction.next(), instruction))
    }

    fn lift_block(
        &self,
        _addr: EVAddr,
        _source: impl TargetInstructionSource,
    ) -> LifterResult<(EVAddr, Vec<TargetInstruction>)> {
        todo!()
    }

    // fn disasm(
    //     &self,
    //     data: &[u8],
    // ) -> JIRLifterResult<(usize, String)> {
    //     todo!()
    // }
    //
    // fn disasm_many(
    //     &self,
    //     data: &[u8],
    // ) -> JIRLifterResult<(usize, Vec<String>)> {
    //     todo!()
    // }
}
