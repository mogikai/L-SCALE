use crate::compiler::InternalCompilerResult;
use crate::compiler::error::InternalError;
use crate::jinst;
use crate::spec::builder::StringId;
use crate::spec::jir::basic_block::TargetBasicBlockBuilder;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::instruction::TargetInstructionBuilder;
use crate::spec::jir::space::JIRSpaceType;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
    JIRVarNodeId,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::field::FieldId;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
};
use crate::spec::moir::table::{
    MOIRExport,
    TableId,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRReference,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecification,
    ProcessorSpecificationError,
};
use fnv::FnvHashMap;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use tracing::debug;

pub enum JIRResolvedValue {
    Value(JIRValue),
    Reference {
        address: JIRValue,
        space: SpaceId,
        size: JIRSize,
    },
}

impl From<JIRConst> for JIRResolvedValue {
    fn from(value: JIRConst) -> Self {
        JIRResolvedValue::Value(value.into())
    }
}

impl From<JIRSlice> for JIRResolvedValue {
    fn from(value: JIRSlice) -> Self {
        JIRResolvedValue::Value(value.into())
    }
}

impl From<JIRValue> for JIRResolvedValue {
    fn from(value: JIRValue) -> Self {
        JIRResolvedValue::Value(value)
    }
}

#[derive(Debug)]
pub struct JIRResolver<'a> {
    constructor: &'a MOIRConstructor,
    values: SecondaryMap<MOIRLocalId, Option<JIRValue>>,
    subtables: FnvHashMap<TableId, JIRResolver<'a>>,
}

impl<'a> JIRResolver<'a> {
    pub fn new(
        constructor: &'a MOIRConstructor,
        mut values: SecondaryMap<MOIRLocalId, Option<JIRValue>>,
        // TODO do we need this at construction time? or can we set this later?
        inst_start: EVAddr,
        inst_next: EVAddr,
        inst_next2: Option<EVAddr>,
    ) -> Self {
        let local_inst_start = constructor.local_scope.inst_start();
        let local_inst_next = constructor.local_scope.inst_next();
        let local_inst_next2 = constructor.local_scope.inst_next2();

        // TODO correct generic
        let addr_bytes = EVAddr::<u8>::bits() / 8;

        values[local_inst_start] =
            Some(JIRValue::Const(JIRConst::new(inst_start.try_into().unwrap(), addr_bytes.try_into().unwrap())));
        values[local_inst_next] =
            Some(JIRValue::Const(JIRConst::new(inst_next.try_into().unwrap(), addr_bytes.try_into().unwrap())));

        if let Some(inst_next2) = inst_next2 {
            values[local_inst_next2] =
                Some(JIRValue::Const(JIRConst::new(inst_next2.try_into().unwrap(), addr_bytes.try_into().unwrap())));
        }

        Self {
            constructor,
            values,
            subtables: Default::default(),
        }
    }

    // TODO remove
    pub(crate) fn print_hierarchy(
        &self,
        depth: u32,
    ) {
        for _ in 0..depth {
            eprint!(" ");
        }

        eprintln!("|-> {:?}", self.constructor.display);
        for subtable_resolver in self.subtables.values() {
            subtable_resolver.print_hierarchy(depth + 4);
        }
    }

    // TODO remove
    pub(crate) fn print_mcode(
        &self,
        spec: &ProcessorSpecification,
        depth: u32,
    ) {
        for _ in 0..depth {
            eprint!(" ");
        }

        eprintln!("{}", self.constructor.mcode_debug_string(&spec.interner));
        for subtable_resolver in self.subtables.values() {
            subtable_resolver.print_mcode(spec, depth + 4);
        }
    }

    /// Resolve the unresolved temporaries for the associated constructor.
    /// This will allocate temp varnodes in the [TargetInstruction] for each temporary
    /// in the scope.
    pub fn realize_temporaries(
        &mut self,
        instruction_builder: &mut TargetInstructionBuilder,
    ) -> ProcessorSpecResult {
        for (local, temp) in self.constructor.local_scope.iter_used_temps() {
            let MOIRMaybeSize::Determined(size) = temp.size else {
                todo!("Generate error for undetermined temporary size")
            };
            let tmp_slice = instruction_builder.alloc_temporary(size)?;
            self.values[local] = Some(tmp_slice.into());
        }
        Ok(())
    }

    pub fn build(
        self,
        spec: &ProcessorSpecification,
        block_builder: &mut TargetBasicBlockBuilder,
    ) -> ProcessorSpecResult {
        self.constructor.generate_jcode(spec, self, block_builder)
    }

    pub fn constructor(&self) -> &MOIRConstructor {
        self.constructor
    }

    /// Assign `value` to the local given by `local_id`. Return the previous value if existing.
    pub fn define_id(
        &mut self,
        local_id: MOIRLocalId,
        value: JIRValue,
    ) {
        self.values[local_id] = Some(value);
    }

    pub fn has_field(
        &self,
        field_id: FieldId,
    ) -> bool {
        self.constructor
            .local_scope
            .local_id_by_field_id(field_id)
            .is_some()
    }

    /// Assign `value` to the local corresponding to `field_id`.
    pub fn define_field(
        &mut self,
        field_id: FieldId,
        value: u64,
    ) -> InternalCompilerResult {
        let local_id = self
            .constructor
            .local_scope
            .local_id_by_field_id(field_id)
            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing field id in scope") })?;

        let field_size = self
            .constructor
            .local_scope
            .field_size(field_id)?
            .determined_or(0);

        let jir_value = self.values[local_id].get_or_insert(JIRValue::Const(JIRConst::new(0, field_size)));
        let JIRValue::Const(c) = jir_value else {
            todo!("Generate error report for slice in field extractor");
        };

        *c.value_mut() = value as u128;
        Ok(())
    }

    /// Return the [MOIRReference::Const] assigned to the local `local_id` or return an error if no
    /// value is assigned.
    pub fn resolve_local_id_to_const(
        &self,
        local_id: MOIRLocalId,
    ) -> InternalCompilerResult<u128> {
        let value = self.values[local_id]
            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing local value") })?;
        Ok(match value {
            JIRValue::Const(c) => c.value(),
            _ => todo!("Generate error report for non-const value in const resolution"),
        })
    }

    pub fn inst_start(&self) -> InternalCompilerResult<EVAddr> {
        let local_id = self.constructor.local_scope.inst_start();
        let value: u64 = self
            .resolve_local_id_to_const(local_id)?
            .try_into()
            .unwrap();
        Ok(value.into())
    }

    pub fn inst_next(&self) -> InternalCompilerResult<EVAddr> {
        let local_id = self.constructor.local_scope.inst_next();
        let value: u64 = self
            .resolve_local_id_to_const(local_id)?
            .try_into()
            .unwrap();
        Ok(value.into())
    }

    pub fn inst_next2(&self) -> InternalCompilerResult<EVAddr> {
        let local_id = self.constructor.local_scope.inst_next2();
        let value: u64 = self
            .resolve_local_id_to_const(local_id)?
            .try_into()
            .unwrap();
        Ok(value.into())
    }

    pub fn define_ident(
        &mut self,
        ident: StringId,
        value: JIRValue,
    ) -> ProcessorSpecResult {
        let local = self
            .constructor
            .local_scope
            .local(ident)
            .ok_or_else(|| todo!("Generate Error"))?;
        if local.refers_to_varnode() {
            todo!("Generate error: Overriding varnodes is disallowed during local resolving")
        }
        // TODO: Write code that checks the size constraints of the value against the defined slice
        let local_id = self
            .constructor
            .local_scope
            .local_id_by_ident(ident)
            .ok_or_else(|| todo!("Generate error"))?;
        assert!(self.values[local_id].is_none());
        self.values[local_id] = Some(value);
        Ok(())
    }

    pub fn define_subtable_resolver(
        &mut self,
        subtable: TableId,
        resolver: JIRResolver<'a>,
    ) -> ProcessorSpecResult {
        self.subtables.insert(subtable, resolver);
        Ok(())
    }

    pub fn build_subtable(
        &mut self,
        spec: &ProcessorSpecification,
        instruction_builder: &mut TargetInstructionBuilder,
        subtable: TableId,
    ) -> ProcessorSpecResult {
        trace_jir!("Building subtable {subtable}");
        let resolver = self
            .subtables
            .get_mut(&subtable)
            .ok_or(ProcessorSpecificationError::UnresolvedSubtable)?;
        trace_jir!("{}", resolver.constructor.mcode_debug_string(&spec.interner));
        resolver
            .constructor
            .generate_jcode_subtable(spec, resolver, instruction_builder)
    }

    pub fn resolve_local(
        &self,
        spec: &ProcessorSpecification,
        instruction_builder: &mut TargetInstructionBuilder,
        local: MOIRLocal,
    ) -> ProcessorSpecResult<JIRResolvedValue> {
        match local {
            MOIRLocal::Local(id) => self.resolve_local_id(spec, instruction_builder, id),
            MOIRLocal::Const(value) => {
                let MOIRConst::Determined(value, size) = value else {
                    todo!("Generate error for undetermined size")
                };
                Ok(JIRConst::new(value, size).into())
            }
        }
    }

    pub fn resolve_local_id(
        &self,
        spec: &ProcessorSpecification,
        instruction_builder: &mut TargetInstructionBuilder,
        local: MOIRLocalId,
    ) -> ProcessorSpecResult<JIRResolvedValue> {
        let value = self
            .constructor
            .local_scope
            .resolve_local(local)
            .map_err(|_| todo!("Map error correctly"))?;
        trace_jir!("Local {local} is {value:?}");
        let start = value.start;
        let end = value
            .end
            .ok_or_else(|| todo!("Generate error for undefined size"))?;
        Ok(match value.reference {
            MOIRReference::VarNode(varnode_id) => {
                if self.values[local].is_some() {
                    todo!("Generate error for overridden varnode.")
                }
                let varnode = spec.varnode_slice_by_id(varnode_id)?;
                JIRSlice::new(varnode_id, varnode.space_id, varnode.offset + start, end).into()
            }
            MOIRReference::Temporary(_) | MOIRReference::Const(_) => match self.values[local] {
                None => todo!("Generate error for unresolved temporary."),
                Some(value) => value.into(),
            },
            MOIRReference::Field(field_id) => {
                let Some(value) = self.values[local] else {
                    debug!("Constructor: {:?}", self.constructor);
                    debug!("Scope is {}", self.constructor.local_scope);
                    debug!("Local id is {local}");
                    todo!("Generate error for a local that cannot be resolved because it was not defined")
                };

                if let JIRValue::Const(c) = value {
                    let field_value = c.value();

                    let field =
                        spec.symbol_tables.fields.get(field_id).ok_or_else(
                            || -> ProcessorSpecificationError { todo!("Generate error for missing field") },
                        )?;
                    if !field.variables.is_empty() {
                        // attach variables => resolve varnode
                        let varnode_id = field.variables[usize::try_from(field_value).unwrap()];
                        let varnode = spec.symbol_tables.varnodes.get(varnode_id).unwrap();

                        let slice = JIRSlice {
                            varnode_id: JIRVarNodeId::VarNode(varnode_id),
                            space_id: varnode.space_id,
                            offset: varnode.offset.try_into().unwrap(),
                            size: varnode.size,
                        };

                        JIRResolvedValue::Value(JIRValue::Slice(slice))
                    } else if !field.values.is_empty() {
                        todo!("values");
                    } else if !field.strings.is_empty() {
                        todo!("display");
                    } else {
                        JIRResolvedValue::Value(JIRValue::Const(JIRConst::new(
                            field_value,
                            field.bits.bytes().unwrap(),
                        )))
                    }
                } else {
                    todo!("Generate error for field resolving to non-const");
                }
            }
            MOIRReference::SubTable(table_id) => {
                let subtable_resolver = self
                    .subtables
                    .get(&table_id)
                    .ok_or_else(|| todo!("Generate error: 'Failed to match subtable'"))?;
                // Check the subtable resolver for the symbol that is exported and resolve it to a JIRValue.
                debug!("Descending into subtable {table_id} to resolve export");
                subtable_resolver.resolve_export(spec, instruction_builder)?
            }
        })
    }

    pub fn resolve_export(
        &self,
        spec: &ProcessorSpecification,
        instruction_builder: &mut TargetInstructionBuilder,
    ) -> ProcessorSpecResult<JIRResolvedValue> {
        let Some(export) = self.constructor.export.as_ref() else {
            todo!("generate error for missing export")
        };
        // If the export is a local, we simply return the resolved local
        // This might mean descending recursively into export resolving.
        match export {
            MOIRExport::Local(local) => self.resolve_local(spec, instruction_builder, *local),
            MOIRExport::MemReference { address, space, size } => {
                // We need to recursively resolve the address, since it might also be a dynamic reference
                let address = self.resolve_local(spec, instruction_builder, *address)?;
                let address = match address {
                    JIRResolvedValue::Value(address) => address,
                    JIRResolvedValue::Reference { address, space, size } => {
                        // If this is a load from the constant space, we can just interpret the address
                        // offset as the constant value. I.e. the position in the const address space
                        // corresponds to its value.
                        // TODO: Replace with api call
                        if let Some(optimized) = self.optimize_const_load(spec, address, space, size)? {
                            return Ok(optimized.into());
                        }

                        // In case of a dynamic reference as address for the current reference (i.e. **addr)
                        // we need to insert a load instruction to resolve the first indirection to *addr
                        // The final load or store operation needs to be inserted by the instruction resolver,
                        // since only it knows whether we need a load or store instruction.
                        let tmp = instruction_builder.alloc_temporary(size)?;
                        instruction_builder.push_jir_instruction(jinst!(tmp = *[space]address))?;
                        // Create an rvalue
                        JIRValue::Slice(tmp)
                    }
                };
                Ok(JIRResolvedValue::Reference {
                    address,
                    space: *space,
                    size: size.ok_or_else(|| todo!("Generate error for missing export size"))?,
                })
            }
        }
    }

    /// Interpret the provided address as a constant. I.e. the address' offset is equal to the const's value
    pub fn optimize_const_load(
        &self,
        spec: &ProcessorSpecification,
        address: JIRValue,
        space: SpaceId,
        size: JIRSize,
    ) -> ProcessorSpecResult<Option<JIRConst>> {
        let JIRSpaceType::Const = spec
            .symbol_tables
            .spaces
            .get(space)
            .expect("This is expected")
            .r#type
        else {
            // Space is not const. Nothing to optimize.
            return Ok(None);
        };
        match address {
            JIRValue::Slice(slice) => {
                // Interpret the offset + start as the constant value.
                Ok(Some(JIRConst::new(slice.offset.into(), size)))
            }
            JIRValue::Const(c) => Ok(Some(JIRConst::new(c.value(), size))),
        }
    }
}
