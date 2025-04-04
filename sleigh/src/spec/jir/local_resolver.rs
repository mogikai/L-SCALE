use crate::compiler::InternalCompilerResult;
use crate::compiler::error::InternalError;
use crate::jinst;
use crate::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
use crate::spec::jir::basic_block::instruction::TargetInstructionBuilder;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::jcode::ContinueLifting;
use crate::spec::jir::space::JIRSpaceType;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::field::FieldId;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRTemporaryId,
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
use mogi_sleigh_parser::StringId;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use std::fmt::Write;
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

#[derive(Debug, Clone)]
pub struct JIRResolver<'a> {
    // TODO maybe move constructor out of here and only pass in when needed?
    constructor: &'a MOIRConstructor,
    constructor_id: ConstructorId,
    values: SecondaryMap<MOIRLocalId, Option<JIRValue>>,
    temporaries: SecondaryMap<MOIRTemporaryId, Option<JIRSlice>>,
    subtables: FnvHashMap<TableId, JIRResolver<'a>>,
}

impl<'a> JIRResolver<'a> {
    pub fn new(
        constructor: &'a MOIRConstructor,
        constructor_id: ConstructorId,
        values: SecondaryMap<MOIRLocalId, Option<JIRValue>>,
    ) -> Self {
        Self {
            constructor,
            constructor_id,
            values,
            temporaries: Default::default(),
            subtables: Default::default(),
        }
    }

    /// Set `inst_start`, `inst_next` and `inst_next2`.
    pub(crate) fn set_inst_addresses(
        &mut self,
        inst_start: EVAddr,
        inst_next: EVAddr,
        inst_next2: Option<EVAddr>,
    ) {
        let local_inst_start = self.constructor.local_scope.inst_start();
        let local_inst_next = self.constructor.local_scope.inst_next();
        let local_inst_next2 = self.constructor.local_scope.inst_next2();

        // TODO correct generic
        let addr_bytes = EVAddr::<u8>::bits() / 8;

        self.values[local_inst_start] =
            Some(JIRValue::Const(JIRConst::new(inst_start.try_into().unwrap(), addr_bytes.try_into().unwrap())));
        self.values[local_inst_next] =
            Some(JIRValue::Const(JIRConst::new(inst_next.try_into().unwrap(), addr_bytes.try_into().unwrap())));

        if let Some(inst_next2) = inst_next2 {
            self.values[local_inst_next2] =
                Some(JIRValue::Const(JIRConst::new(inst_next2.try_into().unwrap(), addr_bytes.try_into().unwrap())));
        }
    }

    pub fn hierarchy_debug_string(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();
        writeln!(out, "Resolver hierarchy:")?;
        self._hierarchy_debug_string(0, &mut out)?;
        Ok(out)
    }

    fn _hierarchy_debug_string(
        &self,
        depth: u32,
        out: &mut String,
    ) -> Result<(), std::fmt::Error> {
        for _ in 0..depth {
            write!(out, " ")?;
        }

        writeln!(out, "|-> {:?}", self.constructor.display_string)?;
        for subtable_resolver in self.subtables.values() {
            subtable_resolver._hierarchy_debug_string(depth + 4, out)?;
        }
        Ok(())
    }

    pub(crate) fn mcode_debug_string(
        &self,
        spec: &ProcessorSpecification,
    ) -> Result<String, std::fmt::Error> {
        let mut out = String::new();
        self._mcode_debug_string(spec, 0, &mut out)?;
        Ok(out)
    }

    fn _mcode_debug_string(
        &self,
        spec: &ProcessorSpecification,
        depth: u32,
        out: &mut String,
    ) -> Result<(), std::fmt::Error> {
        for _ in 0..depth {
            write!(out, " ")?;
        }

        writeln!(out, "{}", self.constructor.mcode_debug_string(&spec.interner))?;
        for subtable_resolver in self.subtables.values() {
            subtable_resolver._mcode_debug_string(spec, depth + 4, out)?
        }
        Ok(())
    }

    /// Resolve the unresolved temporaries for the associated constructor.
    /// This will allocate temp varnodes in the [TargetInstruction] for each temporary
    /// in the scope.
    pub fn realize_temporaries(
        &mut self,
        instruction_builder: &mut TargetInstructionBuilder,
    ) -> ProcessorSpecResult {
        for (temp_id, temp) in self.constructor.local_scope.iter_used_temps() {
            let MOIRMaybeSize::Determined(size) = temp.size else {
                todo!("Generate error for undetermined temporary size")
            };
            let tmp_slice =
                instruction_builder.alloc_temporary(size.try_into().expect("The size must fit into JIRSize"))?;
            self.temporaries[temp_id] = Some(tmp_slice.into());
        }
        Ok(())
    }

    pub fn build(
        self,
        spec: &ProcessorSpecification,
        block_builder: &mut TargetBasicBlockBuilder,
    ) -> ProcessorSpecResult<ContinueLifting> {
        self.constructor.generate_jcode(spec, self, block_builder)
    }

    pub fn build_display(
        &self,
        spec: &ProcessorSpecification,
    ) -> ProcessorSpecResult<String> {
        self.constructor.generate_display(spec, self)
    }

    pub fn constructor(&self) -> &MOIRConstructor {
        self.constructor
    }

    pub(crate) fn constructor_id(&self) -> ConstructorId {
        self.constructor_id
    }

    pub(crate) fn subtables(&self) -> &FnvHashMap<TableId, JIRResolver<'a>> {
        &self.subtables
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
        value: u128,
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

        let jir_value = self.values[local_id].get_or_insert(JIRValue::Const(JIRConst::new(
            0,
            field_size
                .try_into()
                .expect("The size must fit into JIRSize"),
        )));
        let JIRValue::Const(c) = jir_value else {
            todo!("Generate error report for slice in field extractor");
        };

        *c.value_mut() = value;
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
                Ok(JIRConst::new(value, size.try_into().expect("The size must fit into JIRSize")).into())
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
        let end = value.end;
        Ok(match value.reference {
            MOIRReference::VarNode(varnode_id) => {
                if self.values[local].is_some() {
                    todo!("Generate error for overridden varnode.")
                }
                let varnode = spec.varnode_slice_by_id(varnode_id)?;
                JIRSlice::new(
                    varnode.space_id,
                    spec.varnode_to_group[varnode_id].expect("Every varnode must have a group id"),
                    varnode.offset + start,
                    JIRSize::try_from(end.ok_or_else(|| todo!("Generate error for undefined size"))?)
                        .expect("The size must fit into JIRSize")
                        - JIRSize::try_from(start).expect("The size must fit into JIRSize"),
                )
                .into()
            }
            MOIRReference::Temporary(temp_id) => match self.temporaries[temp_id] {
                None => todo!("Generate error for unresolved temporary."),
                Some(value) => value
                    .slice(
                        start,
                        JIRSize::try_from(end.ok_or_else(|| todo!("Generate error for undefined size"))? - start)
                            .expect("The size must fit into JIRSize"),
                    )
                    .into(),
            },
            MOIRReference::Const(_) => match self.values[local] {
                None => todo!("Generate error for unresolved temporary."),
                Some(value) => value.into(),
            },
            MOIRReference::Field(field_id) => {
                let Some(value) = self.values[local] else {
                    debug!("Constructor: {:?}", self.constructor);
                    debug!("Scope is {}", self.constructor.local_scope);
                    debug!("Local id is {local}");
                    debug!("Field id is {field_id}");
                    todo!("Generate error for a local that cannot be resolved because it was not defined")
                };

                if let JIRValue::Const(c) = value {
                    let field_value = c.value();

                    let field = spec
                        .symbol_tables
                        .fields
                        .get(field_id)
                        .ok_or_else(|| -> ProcessorSpecificationError { todo!("Generate error for missing field") })?;
                    if !field.variables.is_empty() {
                        // attach variables => resolve varnode
                        let varnode_id = field.variables[usize::try_from(field_value).unwrap()];
                        let varnode = spec.symbol_tables.varnodes.get(varnode_id).unwrap();

                        let slice = JIRSlice {
                            space_id: varnode.space_id,
                            group_id: Some(
                                spec.varnode_to_group[varnode_id].expect("Every varnode must have a group id"),
                            ),
                            offset: varnode.offset.try_into().unwrap(),
                            size: varnode.size,
                        };

                        JIRResolvedValue::Value(JIRValue::Slice(slice))
                    } else if !field.values.is_empty() {
                        let attached_value = *field
                            .values
                            .get(usize::try_from(field_value).unwrap())
                            .ok_or_else(|| -> ProcessorSpecificationError {
                                todo!("Generate error report for out of bounds attached field")
                            })?;
                        JIRResolvedValue::Value(JIRValue::Const(JIRConst::new(
                            attached_value.into(),
                            field
                                .bits
                                .bits()
                                .try_into()
                                .expect("The size must fit into JIRSize"),
                        )))
                    } else if !field.strings.is_empty() {
                        todo!("display");
                    } else {
                        JIRResolvedValue::Value(JIRValue::Const(JIRConst::new(
                            field_value,
                            field
                                .bits
                                .bits()
                                .try_into()
                                .expect("The size must fit into JIRSize"),
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
                        // TODO move this to a later stage since we can only optimize const loads
                        // that appear on

                        if let Some(optimized) = self.optimize_const_load(spec, address, space, size)? {
                            // space is const
                            optimized.into()
                        } else {
                            // space is not const

                            // In case of a dynamic reference as address for the current reference (i.e. **addr)
                            // we need to insert a load instruction to resolve the first indirection to *addr
                            // The final load or store operation needs to be inserted by the instruction resolver,
                            // since only it knows whether we need a load or store instruction.
                            let tmp = instruction_builder.alloc_temporary(size)?;
                            instruction_builder.push_jir_instruction(jinst!(tmp = *[space]address))?;
                            // Create an rvalue
                            JIRValue::Slice(tmp)
                        }
                    }
                };
                Ok(JIRResolvedValue::Reference {
                    address,
                    space: *space,
                    size: size
                        .ok_or_else(|| todo!("Generate error for missing export size"))?
                        .try_into()
                        .expect("The size must fit into JIRSize"),
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
