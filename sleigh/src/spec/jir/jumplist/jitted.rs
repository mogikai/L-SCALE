// Since the jumplist structure is fixed after creation, we can gain a performance
// boost by jit-compiling certain parts.

// Some considerations:
// - no dynamic containers are needed since the maximum size is known

use std::rc::Rc;

use inkwell::targets::{CodeModel, FileType, RelocMode, Target, TargetMachine};
use inkwell::values::{BasicValue, PointerValue};
use inkwell::{AddressSpace, OptimizationLevel};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use inkwell::module::Module;
use inkwell::types::BasicType;

use crate::compiler::InternalCompilerResult;
use crate::spec::jir::context_provider::ContextProvider;
use crate::spec::moir::context::ContextId;
use crate::spec::moir::table::{MOIRTable, TableId};
use crate::spec::typed_index_table::TypedIndexTable;

use super::jumplist::{
    DataProvider,
    Jumplist,
    JumplistId,
};

type DecodeValueFunc = unsafe extern "C" fn(*const u8, *const ContextProvider) -> u64;
type MatchLocalFunc = unsafe extern "C" fn(*const u8, *const ContextProvider) -> u64;

pub(crate) struct JittedJumplist<'jit> {
    decode_value: JitFunction<'jit, DecodeValueFunc>,
}

impl<'jit> JittedJumplist<'jit> {
    fn new(decode_value: JitFunction<'jit, DecodeValueFunc>) -> Self {
        Self {
            decode_value,
        }
    }

    pub(crate) fn r#match(
        &self,
        bytes: &[u8],
        context_provider: &mut ContextProvider,
    ) {
        let decoded = unsafe {
            self.decode_value.call(bytes.as_ptr(), context_provider as *const ContextProvider)
        };

        eprintln!("decoded = {:#x?}", decoded);
    }
}

pub(crate) struct JittedJumplistBuilder<'a> {
    context: &'a Context,
    module: Module<'a>,
    builder: Builder<'a>,
    execution_engine: ExecutionEngine<'a>,
}

impl<'a> JittedJumplistBuilder<'a> {
    pub(crate) fn new(context: &'a Context, opt_level: OptimizationLevel) -> InternalCompilerResult<Self> {
        let module = context.create_module("jitted_jumplist");
        let builder = context.create_builder();
        let execution_engine = module.create_jit_execution_engine(opt_level).unwrap();

        Ok(Self {
            context: &context,
            module,
            builder,
            execution_engine,
        })
    }

    pub(crate) fn print_module(&self) {
        eprintln!("{}", self.module.to_string());
    }
}

impl JittedJumplistBuilder<'_> {
    fn jit_decode_value(
        &self,
        jumplist: &Jumplist,
        jumplist_id: JumplistId,
    ) -> InternalCompilerResult<JitFunction<DecodeValueFunc>> {
        // declare types and names
        let i8_type = self.context.i8_type();
        let i64_type = self.context.i64_type();
        let ptrsize_type = self.context.ptr_sized_int_type(self.execution_engine.get_target_data(), None);
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let fn_type = i64_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        let fn_name = format!("{jumplist_id}.decode_value");

        // create function and entry block
        let function = self.module.add_function(&fn_name, fn_type, None);
        let param_bytes = function.get_nth_param(0).unwrap().into_pointer_value();
        let param_cp = function.get_nth_param(1).unwrap().into_pointer_value();
        let entry_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_block);

        // emit code
        match jumplist.data_provider {
            DataProvider::ByteStream => {
                let mut result_value = i64_type.const_int(0, false);
                for bytei in 0..jumplist.bytes {
                    let byte_ptr: PointerValue = unsafe {
                        self.builder.build_gep(i8_type, param_bytes, &[ptrsize_type.const_int(bytei.into(), false)], "").unwrap()
                    };

                    let byte = self.builder.build_load(i8_type, byte_ptr, "").unwrap().into_int_value();
                    let extended = self.builder.build_int_z_extend(byte, i64_type, "").unwrap();
                    let shifted = self.builder.build_left_shift(extended, i64_type.const_int(u64::from(bytei) * 8, false), "").unwrap();
                    result_value = self.builder.build_int_add(result_value, shifted, "").unwrap();
                }

                self.builder.build_return(Some(&result_value)).unwrap();
            },
            DataProvider::Context(cid) => {
                let cv_type = i64_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
                let cv_ptr = self.builder.build_int_to_ptr(i64_type.const_int(context_value as u64, false), ptr_type, "").unwrap();

                let result_value = self.builder.build_indirect_call(cv_type, cv_ptr, &[param_cp.into(), i64_type.const_int(usize::from(cid) as u64, false).into()], "").unwrap().try_as_basic_value().left().unwrap();
                self.builder.build_return(Some(&result_value)).unwrap();
            }
        }

        self.print_module();

        // compile
        unsafe { Ok(self.execution_engine.get_function(&fn_name).unwrap()) }
    }

    fn jit_match_local(
        &self,
        jumplist: &Jumplist,
        jumplist_id: JumplistId,
    ) -> InternalCompilerResult<JitFunction<DecodeValueFunc>> {
        todo!("")
    }

    /// JIT-compile the whole jumplist tree.
    pub(crate) fn jit_jumplist_hierarchy(
        &self,
        jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
        tables: &TypedIndexTable<TableId, MOIRTable>,
    ) -> InternalCompilerResult<JittedJumplist> {
        let (root_jumplist_id, root_jumplist) = jumplist_cache.iter().next().unwrap();
        let jitted_decode_value = self.jit_decode_value(&root_jumplist, root_jumplist_id)?;

        let jitted_jumplist = JittedJumplist::new(jitted_decode_value);
        Ok(jitted_jumplist)
    }

    pub(crate) fn dump_machine_code(&self) {
        let triple = TargetMachine::get_default_triple();
        let cpu = TargetMachine::get_host_cpu_name().to_string();
        let features = TargetMachine::get_host_cpu_features().to_string();

        let target = Target::from_triple(&triple).unwrap();
        let machine = target
            .create_target_machine(
                &triple,
                &cpu,
                &features,
                OptimizationLevel::Aggressive,
                RelocMode::Default,
                CodeModel::Default,
            )
            .unwrap();

        machine.write_to_file(&self.module, FileType::Assembly, "out.asm".as_ref()).unwrap();
    }
}

// wrapper functions that simplify certain calls that aren't properly jitted yet
extern "C" fn context_value(
    context_provider: *const ContextProvider,
    context_id: u64,
) -> u64 {
    unsafe {
        (*context_provider).context_value(ContextId::from(context_id as usize)).unwrap()
    }
}
