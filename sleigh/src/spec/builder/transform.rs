use crate::compiler::ast::Spanned;
use crate::compiler::SleighCompilerResult;
use crate::spec::builder::{
    MOIRSymbolId,
    MOIRSymbolTables,
    StringId,
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
use crate::spec::moir::varnode::{
    MOIRVarNode,
    VarNodeId,
};
use crate::spec::typed_index_table::TypedIndexTable;
use crate::spec::{
    Ident,
    JIRSymbolId,
    SymbolTables,
};
use std::collections::HashMap;

#[derive(Debug)]
pub(super) struct MOIRToJIRTransformer {
    moir: MOIRData,
    jir: JIRData,
}

#[derive(Debug)]
pub struct JIRData {
    pub symbol_ids: TypedIndexTable<Ident, JIRSymbolId>,
    pub symbol_strings: HashMap<Ident, StringId>,
    pub symbol_tables: SymbolTables,
    pub string_to_ident_mapping: HashMap<StringId, Ident>,
}

#[derive(Debug)]
struct MOIRData {
    symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
    symbol_strings: HashMap<Ident, Spanned<StringId>>,
    symbol_tables: MOIRSymbolTables,
    string_to_ident_mapping: HashMap<StringId, Ident>,
}

impl MOIRToJIRTransformer {
    pub fn new(
        symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
        symbol_strings: HashMap<Ident, Spanned<StringId>>,
        symbol_tables: MOIRSymbolTables,
        string_to_ident_mapping: HashMap<StringId, Ident>,
    ) -> Self {
        Self {
            moir: MOIRData {
                symbol_ids,
                symbol_strings,
                symbol_tables,
                string_to_ident_mapping,
            },
            jir: JIRData {
                symbol_ids: Default::default(),
                symbol_strings: Default::default(),
                symbol_tables: Default::default(),
                string_to_ident_mapping: Default::default(),
            },
        }
    }
    pub fn transform(mut self) -> SleighCompilerResult<JIRData> {
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
        self.jir.symbol_tables = SymbolTables {
            spaces: transform_spaces(self.moir.symbol_tables.spaces)?,
            varnodes: transform_varnodes(self.moir.symbol_tables.varnodes)?,
        };
        self.jir.string_to_ident_mapping = self.moir.string_to_ident_mapping;
        Ok(self.jir)
    }
}

fn transform_spaces(
    spaces: TypedIndexTable<SpaceId, MOIRSpace>
) -> SleighCompilerResult<TypedIndexTable<SpaceId, JIRSpace>> {
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
    varnodes: TypedIndexTable<VarNodeId, MOIRVarNode>
) -> SleighCompilerResult<TypedIndexTable<VarNodeId, JIRVarNode>> {
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
