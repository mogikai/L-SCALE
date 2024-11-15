//! The build part of the compiler. The functions defined in this file build up the
//! Specification in the [ProcessorSpecificationBuilder] by walking over the abstract
//! syntax tree and processing each item.

pub mod atomize;
pub(crate) mod constructor;
mod r#macro;

use crate::compiler::ast::attach::SASTAttach;
use crate::compiler::ast::define::{
    SASTAddressSpace,
    SASTAddressSpaceAttribute,
    SASTAlignment,
    SASTBitrangeRegister,
    SASTContext,
    SASTDefine,
    SASTEndianness,
    SASTToken,
    SASTUserPcodeOp,
    SASTVarnode,
};
use crate::compiler::ast::item::SASTItem;
use crate::compiler::ast::with::SASTWith;
use crate::compiler::ast::{
    SleighAST,
    Spanned,
    TransmuteIntoVec,
};
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::address_space::{
    MOIRSpace,
    MOIRSpaceIsDefault,
    MOIRSpaceType,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::field::{
    MOIRBitRange,
    MOIRField,
    MOIRFieldParent,
};
use crate::spec::moir::token::MOIRToken;
use crate::spec::moir::varnode::MOIRVarNode;
use crate::spec::ProcessorSpecificationError;
use std::ops::Range;
use tracing::{
    debug,
    trace,
};

impl SleighCompiler {
    /// Build the specification from the provided AST by walking over each node and performing
    /// semantic analysis.
    pub(super) fn build(
        &mut self,
        ast: SleighAST,
    ) -> SleighCompilerResult {
        debug!("The AST has {} toplevel items", ast.items.len());
        self.process_items(ast.items)
        // TODO add pass after all items have been processed to deal with recursive tables
    }

    fn process_items(
        &mut self,
        items: Vec<SASTItem>,
    ) -> SleighCompilerResult {
        for item in items {
            let result = match item {
                SASTItem::Attach(attach) => self.process_attach(attach),
                SASTItem::Constructor(constructor) => self.process_constructor(constructor),
                SASTItem::Define(define) => self.process_define(define),
                SASTItem::Macro(pcode_macro) => self.process_pcode_macro(pcode_macro),
                SASTItem::With(with) => self.process_with(with),
                SASTItem::Error(_) => {
                    panic!()
                }
            };
            self.report_error_or_unwrap(result)?;
        }
        Ok(())
    }

    fn process_attach(
        &mut self,
        attach: Spanned<SASTAttach>,
    ) -> SleighCompilerResult {
        match attach.into_value() {
            SASTAttach::SASTVariable(variable) => {
                let variable = variable.into_value();
                let fields = variable.fields.transmute_into_vec();
                let varnodes = variable.varnodes.transmute_into_vec();

                let result = self.spec_builder.attach_variables(fields, varnodes);
                self.report_error_or_unwrap(result)?;
            }
            SASTAttach::SASTValue(value) => {
                let value = value.into_value();
                let fields = value.fields.transmute_into_vec();
                let values = value.values.transmute_into_vec();

                let result = self.spec_builder.attach_values(fields, values);
                self.report_error_or_unwrap(result)?;
            }
            SASTAttach::SASTName(name) => {
                let name = name.into_value();
                let fields = name.fields.transmute_into_vec();
                let strings = name.strings.transmute_into_vec();

                let result = self.spec_builder.attach_names(fields, strings);
                self.report_error_or_unwrap(result)?;
            }
        }
        Ok(())
    }

    fn process_define(
        &mut self,
        define: Spanned<SASTDefine>,
    ) -> SleighCompilerResult {
        match define.into_value() {
            SASTDefine::Endianness(endianness) => self.process_define_endianness(endianness),
            SASTDefine::Alignment(alignment) => self.process_define_alignment(alignment),
            SASTDefine::AddressSpace(space) => self.process_define_space(space),
            SASTDefine::Varnode(varnodes) => self.process_define_varnodes(varnodes),
            SASTDefine::BitrangeRegister(bitrange_register) => {
                self.process_define_bitrange_registers(bitrange_register)
            }
            SASTDefine::UserPcodeOp(pcode_op) => self.process_define_pcode_op(pcode_op),
            SASTDefine::Token(token) => self.process_define_token(token),
            SASTDefine::Context(context) => self.process_define_context(context),
            SASTDefine::Invalid => panic!("This should have been handled earlier."),
        }
    }

    fn process_define_pcode_op(
        &mut self,
        pcode_op: Spanned<SASTUserPcodeOp>,
    ) -> SleighCompilerResult {
        let (pcode_op, _) = pcode_op.into_tuple();
        let result = self
            .spec_builder
            .declare_custom_op(pcode_op.name.transmute());
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    fn process_define_varnodes(
        &mut self,
        varnodes: Spanned<SASTVarnode>,
    ) -> SleighCompilerResult {
        let (varnodes, _) = varnodes.into_tuple();
        let result = self
            .spec_builder
            .space_id_by_ident(varnodes.spacename.transmute());

        let Some(space) = self.report_error_or_unwrap(result)? else {
            // We reported an error already. Quit here for this define and try to continue.
            return Ok(());
        };

        for (i, name) in varnodes.names.into_iter().enumerate() {
            if self.spec_builder.is_placeholder_ident((*name).into())? {
                // Placeholders are skipped.
                continue;
            }
            let varnode = MOIRVarNode {
                space,
                offset: varnodes.offset.value() + (i as u64 * u64::from(*varnodes.size)),
                size: *varnodes.size,
            };
            let result = self.spec_builder.define_varnode(name.transmute(), varnode);
            self.report_error_or_unwrap(result)?;
        }
        Ok(())
    }

    fn process_define_bitrange_registers(
        &mut self,
        bitrange_registers: Vec<Spanned<SASTBitrangeRegister>>,
    ) -> SleighCompilerResult {
        for bitrange_register in bitrange_registers {
            let bitrange_register = bitrange_register.into_value();
            let result = self.spec_builder.define_bitrange_register(
                bitrange_register.name.transmute(),
                bitrange_register.source_varnode.transmute(),
                bitrange_register.offset,
                bitrange_register.size,
            );
            self.report_error_or_unwrap(result)?;
        }

        Ok(())
    }

    fn process_define_space(
        &mut self,
        space: Spanned<SASTAddressSpace>,
    ) -> SleighCompilerResult {
        let (space, _) = space.into_tuple();
        let mut r#type = None;
        let mut address_size = None;
        let mut word_size = None;
        let mut default = MOIRSpaceIsDefault::NotDefault;
        for (attribute, _) in space.attributes.into_iter().map(Spanned::into_tuple) {
            match attribute {
                SASTAddressSpaceAttribute::Type(ty) => {
                    if r#type.is_some() {
                        todo!("Generate error report for duplicate space type attribute.")
                    } else {
                        r#type = Some(ty.into());
                    }
                }
                SASTAddressSpaceAttribute::Size(size) => {
                    if address_size.is_some() {
                        todo!("Generate error report for duplicate size attribute.")
                    } else {
                        address_size = Some(size);
                    }
                }
                SASTAddressSpaceAttribute::Wordsize(size) => {
                    if word_size.is_some() {
                        todo!("Generate error report for duplicate word size attribute.")
                    } else {
                        word_size = Some(size);
                    }
                }
                SASTAddressSpaceAttribute::IsDefault => {
                    if matches!(default, MOIRSpaceIsDefault::Default) {
                        todo!("Generate error report for duplicate default attribute.")
                    } else {
                        default = MOIRSpaceIsDefault::Default;
                    }
                }
            }
        }
        let Some(address_size) = address_size else {
            todo!("Generate error report for missing size attribute.");
        };
        let moir_space = MOIRSpace {
            r#type: r#type.unwrap_or(MOIRSpaceType::Ram),
            address_size,
            wordsize: word_size.unwrap_or(1),
        };
        self.spec_builder
            .define_space(space.name.transmute(), moir_space, default)?;
        Ok(())
    }

    fn process_define_token(
        &mut self,
        token: Spanned<SASTToken>,
    ) -> SleighCompilerResult {
        // We first define the token and then we can define its fields, since they need to refer to the token.
        let (token, _) = token.into_tuple();
        let bits = token.bits.into_value();
        if bits % 8 != 0 {
            Err(ProcessorSpecificationError::TokenSizeNotAMultipleOfEightBit)?;
        }
        let result = self.spec_builder.define_token(
            token.name.transmute(),
            MOIRToken {
                bytes: MOIRSize::try_from(bits).map_err(|_| SleighCompilerError::TODO)? / 8,
                endianness_override: token.endian_override.map(|e| e.into_value().into()),
            },
        );
        if let Some(token_id) = self.report_error_or_unwrap(result)? {
            for field in token.fields.into_iter() {
                let name = field.name.clone().transmute();
                // +1 since end is inclusive, but `MOIRBitRange` is exclusive
                let range = MOIRBitRange::new(*field.start.value() as u8, *field.end.value() as u8 + 1);
                if let Some(field) = self.make_field(MOIRFieldParent::Token(token_id), bits, range)? {
                    let result = self.spec_builder.define_field(name, field);
                    self.report_error_or_unwrap(result)?;
                }
            }
        }
        Ok(())
    }

    fn process_define_context(
        &mut self,
        context: Spanned<SASTContext>,
    ) -> SleighCompilerResult {
        let (context_id, bits) = self
            .spec_builder
            .define_context(context.varnode.clone().transmute())?;
        for field in &context.fields {
            // +1 since end is inclusive, but `MOIRBitRange` is exclusive
            let range = MOIRBitRange::new(*field.start.value() as u8, *field.end.value() as u8 + 1);
            if let Some(moir_field) = self.make_field(MOIRFieldParent::Context(context_id), bits, range)? {
                let result = self
                    .spec_builder
                    .define_field(field.name.clone().transmute(), moir_field);
                self.report_error_or_unwrap(result)?;
            }
        }

        Ok(())
    }

    /// Create a new field instance to be defined in the [ProcessorSpecificationBuilder].
    /// The field is associated with the provided `parent`.
    /// This function also performs semantic checks, and thus needs the amount of bits
    /// the token contains.
    fn make_field(
        &mut self,
        parent: MOIRFieldParent,
        bits: u64,
        field_range: MOIRBitRange,
    ) -> Result<Option<MOIRField>, SleighCompilerError> {
        if field_range.start > field_range.end {
            todo!("Generate correct error report and return Ok(None)")
        } else if field_range.bits() > MOIRSize::try_from(bits).unwrap() {
            todo!("Generate correct error report and return Ok(None)")
        } else {
            // The variables, values, and strings fields are later attached with attach statements.
            let field = MOIRField {
                parent,
                bits: field_range,
                variables: vec![],
                values: vec![],
                strings: vec![],
            };
            Ok(Some(field))
        }
    }

    fn process_define_endianness(
        &mut self,
        endianness: Spanned<SASTEndianness>,
    ) -> SleighCompilerResult {
        let result = self.spec_builder.define_endianness(endianness.transmute());
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    fn process_define_alignment(
        &mut self,
        alignment: Spanned<SASTAlignment>,
    ) -> SleighCompilerResult {
        let result = self.spec_builder.define_alignment(alignment.transmute());
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    fn process_with(
        &mut self,
        with: Spanned<SASTWith>,
    ) -> SleighCompilerResult {
        trace!("Processing with statement at {:?}.", with.span());
        self.process_items(with.into_value().items)?;
        Ok(())
    }
}

#[cfg(feature = "trace_build")]
macro_rules! trace_build {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*)
    };
}
#[cfg(not(feature = "trace_build"))]
macro_rules! trace_build {
    ($($arg:tt)*) => {};
}

pub(crate) use trace_build;
