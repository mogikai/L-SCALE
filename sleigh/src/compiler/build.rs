//! The build part of the compiler. The functions defined in this file build up the
//! Specification in the [ProcessorSpecificationBuilder] by walking over the abstract
//! syntax tree and processing each item.

pub mod atomize;
pub(crate) mod constructor;
mod r#macro;

use crate::compiler::error::{
    DuplicatedSpaceAttribute,
    InternalError,
    InvalidFieldRange,
};
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::ProcessorSpecificationError;
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
use mogi_sleigh_parser::ast::attach::SASTAttach;
use mogi_sleigh_parser::ast::define::{
    SASTAddressSpace,
    SASTAddressSpaceAttribute,
    SASTAlignment,
    SASTBitrangeRegister,
    SASTContext,
    SASTContextFieldAttribute,
    SASTDefine,
    SASTEndianness,
    SASTToken,
    SASTTokenFieldAttribute,
    SASTUserPcodeOp,
    SASTVarnode,
};
use mogi_sleigh_parser::ast::item::SASTItem;
use mogi_sleigh_parser::ast::with::SASTWith;
use mogi_sleigh_parser::{
    SleighAST,
    Spanned,
    TransmuteIntoVec,
};
use tracing::debug;

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

impl SleighCompiler {
    /// Build the specification from the provided AST by walking over each node and performing
    /// semantic analysis.
    pub(super) fn build(
        &mut self,
        ast: SleighAST,
    ) -> InternalCompilerResult {
        debug!("The AST has {} toplevel items", ast.items.len());
        self.process_items(ast.items)
    }

    fn process_items(
        &mut self,
        items: Vec<SASTItem>,
    ) -> InternalCompilerResult {
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
    ) -> InternalCompilerResult {
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
    ) -> InternalCompilerResult {
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
    ) -> InternalCompilerResult {
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
    ) -> InternalCompilerResult {
        let (varnodes, _) = varnodes.into_tuple();
        // TODO is the question mark here correct?
        let result = self
            .spec_builder
            .space_id_by_ident(&varnodes.spacename.transmute());

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
                size: varnodes
                    .size
                    .into_value()
                    .try_into()
                    .expect("The size must fit into MOIRSize"),
            };
            let result = self.spec_builder.define_varnode(name.transmute(), varnode);
            self.report_error_or_unwrap(result)?;
        }
        Ok(())
    }

    fn process_define_bitrange_registers(
        &mut self,
        bitrange_registers: Vec<Spanned<SASTBitrangeRegister>>,
    ) -> InternalCompilerResult {
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
    ) -> InternalCompilerResult {
        let (space, _) = space.into_tuple();
        let mut r#type = None;
        let mut address_size = None;
        let mut word_size = None;
        let mut default = (MOIRSpaceIsDefault::NotDefault, None);
        for (attribute, input) in space.attributes.into_iter().map(Spanned::into_tuple) {
            match attribute {
                SASTAddressSpaceAttribute::Type(ty) => {
                    if let Some((_, prev_input)) = r#type {
                        return Err(InternalError::from_error((DuplicatedSpaceAttribute { attribute }).into())
                            .span("first", prev_input)
                            .span("second", input));
                    } else {
                        r#type = Some((ty.into(), input));
                    }
                }
                SASTAddressSpaceAttribute::Size(size) => {
                    if let Some((_, prev_input)) = address_size {
                        return Err(InternalError::from_error((DuplicatedSpaceAttribute { attribute }).into())
                            .span("first", prev_input)
                            .span("second", input));
                    } else {
                        address_size = Some((size, input));
                    }
                }
                SASTAddressSpaceAttribute::Wordsize(size) => {
                    if let Some((_, prev_input)) = word_size {
                        return Err(InternalError::from_error((DuplicatedSpaceAttribute { attribute }).into())
                            .span("first", prev_input)
                            .span("second", input));
                    } else {
                        word_size = Some((size, input));
                    }
                }
                SASTAddressSpaceAttribute::IsDefault => {
                    if let (_, Some(prev_input)) = default {
                        return Err(InternalError::from_error((DuplicatedSpaceAttribute { attribute }).into())
                            .span("first", prev_input)
                            .span("second", input));
                    } else {
                        default = (MOIRSpaceIsDefault::Default, Some(input));
                    }
                }
            }
        }
        let moir_space = MOIRSpace {
            r#type: r#type.map(|t| t.0).unwrap_or(MOIRSpaceType::Ram),
            address_size: address_size
                .map(|t| t.0.try_into().expect("The size must fit into MOIRSize"))
                .unwrap(),
            wordsize: word_size
                .map(|t| t.0.try_into().expect("The size must fit into MOIRSize"))
                .unwrap_or(1),
        };
        self.spec_builder
            .define_space(space.name.transmute(), moir_space, default.0)?;
        Ok(())
    }

    fn process_define_token(
        &mut self,
        token: Spanned<SASTToken>,
    ) -> InternalCompilerResult {
        // We first define the token and then we can define its fields, since they need to refer to the token.
        let (token, _) = token.into_tuple();
        let bits = token.bits.into_value();
        if bits % 8 != 0 {
            Err(ProcessorSpecificationError::TokenSizeNotAMultipleOfEightBit)?;
        }
        let result = self.spec_builder.define_token(
            token.name.transmute(),
            MOIRToken {
                bytes: MOIRSize::try_from(bits).map_err(|_| -> InternalError { todo!() })? / 8,
                endianness_override: token.endian_override.map(|e| e.into_value().into()),
            },
        );
        if let Some(token_id) = self.report_error_or_unwrap(result)? {
            for field in token.fields.into_iter() {
                let name = field.name.transmute();
                // +1 since end is inclusive, but `MOIRBitRange` is exclusive
                let range = MOIRBitRange::new(
                    MOIRSize::try_from(*field.start.value()).unwrap(),
                    MOIRSize::try_from(*field.end.value() + 1).unwrap(),
                );

                let mut signed = false;
                let mut hex = false;
                for attr in field.attributes.iter() {
                    match attr.value() {
                        SASTTokenFieldAttribute::Signed => signed = true,
                        SASTTokenFieldAttribute::Hex => hex = true,
                        SASTTokenFieldAttribute::Dec => todo!("Generate error report for unsupported dec attribute"),
                    }
                }

                if let Some(field) = self
                    .make_field(MOIRFieldParent::Token(token_id), bits, range, signed, hex)
                    .map_err(|e| e.span("field", *field.span()))?
                {
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
    ) -> InternalCompilerResult {
        let (context_id, bits) = self
            .spec_builder
            .define_context(context.varnode.transmute())?;
        for field in &context.fields {
            // +1 since end is inclusive, but `MOIRBitRange` is exclusive
            let range = MOIRBitRange::new(
                // inverted bit range since context works the other way around
                MOIRSize::try_from(bits - *field.end.value() - 1).unwrap(),
                MOIRSize::try_from(bits - *field.start.value()).unwrap(),
            );

            let mut signed = false;
            let mut hex = false;
            for attr in field.attributes.iter() {
                match attr.value() {
                    SASTContextFieldAttribute::Signed => signed = true,
                    SASTContextFieldAttribute::Hex => hex = true,
                    SASTContextFieldAttribute::Dec => todo!("Generate error report for unsupported dec attribute"),
                    SASTContextFieldAttribute::Noflow => todo!("Noflow"),
                }
            }

            if let Some(moir_field) = self
                .make_field(MOIRFieldParent::Context(context_id), bits, range, signed, hex)
                .map_err(|e| e.span("field", *field.span()))?
            {
                let result = self
                    .spec_builder
                    .define_field(field.name.transmute(), moir_field);
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
        signed: bool,
        hex: bool,
    ) -> InternalCompilerResult<Option<MOIRField>> {
        if field_range.start > field_range.end {
            Err(InternalError::from_error(
                InvalidFieldRange::EndBeforeStart(field_range.start as u64..field_range.end as u64).into(),
            ))
        } else if field_range.bits() > MOIRSize::try_from(bits).unwrap() {
            Err(InternalError::from_error(
                InvalidFieldRange::TooLargeRange(bits, field_range.start as u64..field_range.end as u64).into(),
            ))
        } else {
            // The variables, values, and strings fields are later attached with attach statements.
            let field = MOIRField {
                parent,
                bits: field_range,
                variables: vec![],
                values: vec![],
                strings: vec![],
                signed,
                hex,
            };

            Ok(Some(field))
        }
    }

    fn process_define_endianness(
        &mut self,
        endianness: Spanned<SASTEndianness>,
    ) -> InternalCompilerResult {
        let result = self.spec_builder.define_endianness(endianness.transmute());
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    fn process_define_alignment(
        &mut self,
        alignment: Spanned<SASTAlignment>,
    ) -> InternalCompilerResult {
        let result = self.spec_builder.define_alignment(alignment.transmute());
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    fn process_with(
        &mut self,
        with: Spanned<SASTWith>,
    ) -> InternalCompilerResult {
        trace_build!("Processing with statement at {:?}.", with.span());
        self.process_items(with.into_value().items)?;
        Ok(())
    }
}
