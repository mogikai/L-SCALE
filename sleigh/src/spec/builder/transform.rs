use crate::compiler::InternalCompilerResult;

use crate::compiler::ast::Spanned;
use crate::spec::builder::{
    CustomInterner,
    MOIRSymbolId,
    MOIRSymbolTables,
    StringId,
};
use crate::spec::jir::custom_op::JIRCustomOp;
use crate::spec::jir::jumplist::{
    Jumplist,
    JumplistId,
};
use crate::spec::jir::space::{
    JIRSpace,
    JIRSpaceType,
};
use crate::spec::jir::values::JIRVarNode;
use crate::spec::moir::address_space::{
    MOIRSpace,
    MOIRSpaceType,
    SpaceId,
};
use crate::spec::moir::context::{
    ContextId,
    MOIRContext,
};
use crate::spec::moir::custom_op::MOIRCustomOp;
use crate::spec::moir::table::TableId;
use crate::spec::moir::varnode::{
    MOIRVarNode,
    VarNodeId,
};
use crate::spec::typed_index_table::TypedIndexTable;
use crate::spec::{
    CustomOpId,
    Ident,
    JIRSymbolId,
    JIRSymbolTables,
};
use mogi_utilities::index_map::PrimaryMap;
use std::collections::HashMap;

use self::jumplist::JumplistBuilder;

mod jumplist;
use string_interner::StringInterner;
use string_interner::backend::StringBackend;
use tracing::warn;

#[derive(Debug)]
pub(super) struct MOIRToJIRTransformer<'a> {
    moir: MOIRData<'a>,
    jir: JIRData,
}

#[derive(Debug)]
pub struct JIRData {
    pub symbol_ids: TypedIndexTable<Ident, JIRSymbolId>,
    pub symbol_strings: HashMap<Ident, StringId>,
    pub symbol_tables: JIRSymbolTables,
    pub jumplist_cache: TypedIndexTable<JumplistId, Jumplist>,
    pub string_to_ident_mapping: HashMap<StringId, Ident>,
}

#[derive(Debug)]
struct MOIRData<'a> {
    interner: &'a StringInterner<StringBackend>,
    symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
    symbol_strings: HashMap<Ident, Spanned<StringId>>,
    symbol_tables: MOIRSymbolTables,
    string_to_ident_mapping: HashMap<StringId, Ident>,
    context: &'a TypedIndexTable<ContextId, MOIRContext>,
    root_table_id: TableId,
}

impl<'a> MOIRToJIRTransformer<'a> {
    pub fn new(
        interner: &'a StringInterner<StringBackend>,
        symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
        symbol_strings: HashMap<Ident, Spanned<StringId>>,
        symbol_tables: MOIRSymbolTables,
        string_to_ident_mapping: HashMap<StringId, Ident>,
        context: &'a TypedIndexTable<ContextId, MOIRContext>,
        root_table_id: TableId,
    ) -> Self {
        Self {
            moir: MOIRData {
                interner,
                symbol_ids,
                symbol_strings,
                symbol_tables,
                string_to_ident_mapping,
                context,
                root_table_id,
            },
            jir: JIRData {
                symbol_ids: Default::default(),
                symbol_strings: Default::default(),
                symbol_tables: Default::default(),
                jumplist_cache: Default::default(),
                string_to_ident_mapping: Default::default(),
            },
        }
    }

    pub fn transform(mut self) -> InternalCompilerResult<JIRData> {
        self.jir.symbol_ids = self
            .moir
            .symbol_ids
            .into_iter()
            .map(|(ident, symbol_id)| {
                let symbol_id = match symbol_id {
                    MOIRSymbolId::Undefined => todo!("Generate error for undefined symbol"),
                    MOIRSymbolId::Macro(id) => JIRSymbolId::Macro(id),
                    MOIRSymbolId::Space(id) => JIRSymbolId::Space(id),
                    MOIRSymbolId::Table(id) => JIRSymbolId::Table(id),
                    MOIRSymbolId::Token(id) => JIRSymbolId::Token(id),
                    MOIRSymbolId::Field(id) => JIRSymbolId::Field(id),
                    MOIRSymbolId::VarNode(id) => JIRSymbolId::VarNode(id),
                    MOIRSymbolId::BitrangeRegister(id) => JIRSymbolId::BitrangeRegister(id),
                    MOIRSymbolId::CustomOp(id) => JIRSymbolId::CustomOp(id),
                };
                (ident, symbol_id)
            })
            .collect();
        self.jir.symbol_strings = self
            .moir
            .symbol_strings
            .into_iter()
            .map(|(k, v)| (k, v.into_value()))
            .collect();

        let jumplist_cache = JumplistBuilder::new(
            &self.moir.symbol_tables.tables,
            &self.moir.symbol_tables.fields,
            &self.moir.symbol_tables.tokens,
            &self.moir.context,
            &self.moir.symbol_tables.varnodes,
            &self.moir.symbol_tables.constructors,
        )
        .transform(self.moir.root_table_id)?;

        self.jir.jumplist_cache = jumplist_cache;

        self.jir.symbol_tables = JIRSymbolTables {
            spaces: transform_spaces(self.moir.symbol_tables.spaces)?,
            tables: self.moir.symbol_tables.tables,
            constructors: self.moir.symbol_tables.constructors,
            varnodes: transform_varnodes(self.moir.symbol_tables.varnodes)?,
            custom_ops: transform_custom_ops(self.moir.interner, self.moir.symbol_tables.custom_ops)?,
            fields: self.moir.symbol_tables.fields,
        };
        self.jir.string_to_ident_mapping = self.moir.string_to_ident_mapping;
        Ok(self.jir)
    }
}

fn transform_custom_ops(
    interner: &StringInterner<StringBackend>,
    custom_ops: PrimaryMap<CustomOpId, MOIRCustomOp>,
) -> InternalCompilerResult<PrimaryMap<CustomOpId, JIRCustomOp>> {
    Ok(custom_ops
        .into_iter()
        .filter_map(|(id, op)| match op {
            MOIRCustomOp::Declared { name } => {
                let name = interner
                    .string(*name)
                    .expect("The string has to exist since we saved an id to it.");
                // TODO: Emit a compiler warning instead of a log output
                warn!("Custom op '{name}' is declared but not defined. Ignoring.");
                None
            }
            MOIRCustomOp::Defined { name, .. } => Some((id, JIRCustomOp {
                name: interner
                    .string(*name)
                    .expect("The string has to exist since we saved an id to it.")
                    .to_string(),
            })),
        })
        .collect::<PrimaryMap<CustomOpId, JIRCustomOp>>())
}

fn transform_spaces(
    spaces: TypedIndexTable<SpaceId, MOIRSpace>
) -> InternalCompilerResult<TypedIndexTable<SpaceId, JIRSpace>> {
    Ok(spaces
        .into_iter()
        .map(|(ident, space)| {
            let r#type = match space.r#type {
                MOIRSpaceType::Ram => JIRSpaceType::Ram,
                MOIRSpaceType::Rom => JIRSpaceType::Rom,
                MOIRSpaceType::Register => JIRSpaceType::Register,
                MOIRSpaceType::Const => JIRSpaceType::Const,
            };
            let space = JIRSpace {
                r#type,
                address_size: space.address_size,
                wordsize: space.wordsize,
            };
            (ident, space)
        })
        .collect())
}

fn transform_varnodes(
    varnodes: PrimaryMap<VarNodeId, MOIRVarNode>
) -> InternalCompilerResult<PrimaryMap<VarNodeId, JIRVarNode>> {
    Ok(varnodes
        .into_iter()
        .map(|(ident, varnode)| {
            let varnode = JIRVarNode {
                space_id: varnode.space,
                offset: varnode.offset,
                size: varnode.size,
            };
            (ident, varnode)
        })
        .collect())
}
