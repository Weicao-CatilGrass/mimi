use crate::ast::*;
use crate::codegen::types;
use crate::codegen::{call_try_basic_value, CallSiteValueExt, CodeGenerator, VarEntry};
use crate::error::CompileError;

use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};
use std::collections::{BTreeMap, HashMap};

impl<'ctx> CodeGenerator<'ctx> {

    /// Compile an expression, falling back to a function reference if the name is a module function.
    pub(in crate::codegen) fn compile_expr_or_func_ref(
        &mut self,
        expr: &Expr,
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        match self.compile_expr(expr, vars) {
            Ok(val) => Ok(val),
            Err(_) => {
                // Try to resolve as a module function
                if let Expr::Ident(name) = expr {
                    if let Some(func) = self.module.get_function(name) {
                        let fn_ptr = func.as_global_value().as_pointer_value();
                        return Ok(BasicValueEnum::PointerValue(fn_ptr));
                    }
                }
                // Re-compile to get the original error
                self.compile_expr(expr, vars)
            }
        }
    }


    /// Call a function reference (named function or closure) with a single i64 argument.
    /// Returns the call result (i64 for map-style, struct for and_then-style).
    pub(in crate::codegen) fn compile_call_fn_ref(
        &mut self,
        fn_ref: BasicValueEnum<'ctx>,
        arg_expr: &Expr,
        payload: BasicValueEnum<'ctx>,
        i64_ty: inkwell::types::IntType<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        match fn_ref {
            BasicValueEnum::StructValue(sv) => {
                let fn_ptr = self.builder.build_extract_value(sv, 0, "fn_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract fn_ptr error: {}", e)))?.into_pointer_value();
                let env_ptr = self.builder.build_extract_value(sv, 1, "env_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract env_ptr error: {}", e)))?.into_pointer_value();
                let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let fn_type = i64_ty.fn_type(&[
                    BasicMetadataTypeEnum::PointerType(i8_ptr),
                    BasicMetadataTypeEnum::IntType(i64_ty),
                ], false);
                let fn_typed = self.builder.build_pointer_cast(
                    fn_ptr, fn_type.ptr_type(inkwell::AddressSpace::default()), "fn_typed"
                ).map_err(|e| CompileError::LlvmError(format!("pointer cast error: {}", e)))?;
                let call = self.builder.build_indirect_call(
                    fn_type, fn_typed, &[
                        BasicMetadataValueEnum::PointerValue(env_ptr),
                        BasicMetadataValueEnum::IntValue(payload.into_int_value()),
                    ], "fn_call"
                ).map_err(|e| CompileError::LlvmError(format!("indirect call error: {}", e)))?;
                Ok(call_try_basic_value(&call).unwrap_or(
                    BasicValueEnum::IntValue(i64_ty.const_int(0, false))
                ))
            }
            BasicValueEnum::PointerValue(pv) => {
                if let Expr::Ident(fn_name) = arg_expr {
                    if let Some(func) = self.module.get_function(fn_name) {
                        let call = self.builder.build_call(func, &[
                            BasicMetadataValueEnum::IntValue(payload.into_int_value()),
                        ], "fn_call")
                            .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?;
                        return Ok(call_try_basic_value(&call).unwrap_or(
                            BasicValueEnum::IntValue(i64_ty.const_int(0, false))
                        ));
                    }
                }
                let closure_struct_ty = self.context.struct_type(&[
                    BasicTypeEnum::PointerType(self.context.i8_type().ptr_type(inkwell::AddressSpace::default())),
                    BasicTypeEnum::PointerType(self.context.i8_type().ptr_type(inkwell::AddressSpace::default())),
                ], false);
                let loaded = self.builder.build_load(BasicTypeEnum::StructType(closure_struct_ty), pv, "closure_loaded")
                    .map_err(|e| CompileError::LlvmError(format!("load closure error: {}", e)))?.into_struct_value();
                let fn_ptr = self.builder.build_extract_value(loaded, 0, "fn_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract fn_ptr error: {}", e)))?.into_pointer_value();
                let env_ptr = self.builder.build_extract_value(loaded, 1, "env_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract env_ptr error: {}", e)))?.into_pointer_value();
                let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let fn_type = i64_ty.fn_type(&[
                    BasicMetadataTypeEnum::PointerType(i8_ptr),
                    BasicMetadataTypeEnum::IntType(i64_ty),
                ], false);
                let fn_typed = self.builder.build_pointer_cast(
                    fn_ptr, fn_type.ptr_type(inkwell::AddressSpace::default()), "fn_typed"
                ).map_err(|e| CompileError::LlvmError(format!("pointer cast error: {}", e)))?;
                let call = self.builder.build_indirect_call(
                    fn_type, fn_typed, &[
                        BasicMetadataValueEnum::PointerValue(env_ptr),
                        BasicMetadataValueEnum::IntValue(payload.into_int_value()),
                    ], "fn_call"
                ).map_err(|e| CompileError::LlvmError(format!("indirect call error: {}", e)))?;
                Ok(call_try_basic_value(&call).unwrap_or(
                    BasicValueEnum::IntValue(i64_ty.const_int(0, false))
                ))
            }
            _ => Err("function reference must be a closure or function pointer".into()),
        }
    }

    /// Handle compile-time builtins that are resolved at codegen time
    /// rather than generating function calls.
    pub(in crate::codegen) fn compile_builtin_intrinsic(
        &mut self,
        name: &str,
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        match name {
            "type_name" if args.len() == 1 => {
                let type_str = match &args[0] {
                    Expr::Ident(var_name) => self.var_type_names.get(var_name)
                        .cloned().unwrap_or_else(|| "unknown".to_string()),
                    Expr::Literal(Lit::String(s)) => s.clone(),
                    _ => "unknown".to_string(),
                };
                // Build string literal: { i8*, i64 }
                let global = self.builder.build_global_string_ptr(&type_str, "type_name")
                    .map_err(|e| CompileError::LlvmError(format!("global string error: {}", e)))?;
                let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let string_ty = self.context.struct_type(&[
                    BasicTypeEnum::PointerType(i8_ptr),
                    BasicTypeEnum::IntType(self.context.i64_type()),
                ], false);
                let alloca = self.builder.build_alloca(string_ty, "type_str")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                let ptr_gep = self.builder.build_struct_gep(string_ty, alloca, 0, "ptr")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(ptr_gep, global.as_pointer_value())
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let len_gep = self.builder.build_struct_gep(string_ty, alloca, 1, "len")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let len = self.context.i64_type().const_int(type_str.len() as u64, false);
                self.builder.build_store(len_gep, len)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                Ok(alloca.into())
            }
            "type_fields" if args.len() == 1 => {
                let type_name_str = match &args[0] {
                    Expr::Literal(Lit::String(s)) => s.clone(),
                    Expr::Ident(var) => self.var_type_names.get(var)
                        .cloned().unwrap_or_else(|| "unknown".to_string()),
                    _ => return Err("type_fields: argument must be a type name string".into()),
                };
                let field_names: Vec<String> = self.type_defs.get(&type_name_str)
                    .map(|td| match &td.kind {
                        TypeDefKind::Record(fields) => {
                            fields.iter().map(|f| f.name.clone()).collect()
                        }
                        TypeDefKind::Enum(variants) => {
                            variants.iter().map(|v| v.name.clone()).collect()
                        }
                        _ => vec![],
                    })
                    .unwrap_or_default();
                // Build a List of field names
                self.build_string_list(&field_names, vars)
            }
            "type_variants" if args.len() == 1 => {
                let type_name_str = match &args[0] {
                    Expr::Literal(Lit::String(s)) => s.clone(),
                    Expr::Ident(var) => self.var_type_names.get(var)
                        .cloned().unwrap_or_else(|| "unknown".to_string()),
                    _ => return Err("type_variants: argument must be a type name string".into()),
                };
                let variant_names: Vec<String> = self.type_defs.get(&type_name_str)
                    .map(|td| match &td.kind {
                        TypeDefKind::Enum(variants) => {
                            variants.iter().map(|v| v.name.clone()).collect()
                        }
                        _ => vec![],
                    })
                    .unwrap_or_default();
                self.build_string_list(&variant_names, vars)
            }
            "keys" | "values" if args.len() == 1 => {
                let var_name = match &args[0] {
                    Expr::Ident(n) => n.clone(),
                    _ => return Err("keys/values: argument must be a variable name".into()),
                };
                let type_name = self.var_type_names.get(&var_name)
                    .cloned().unwrap_or_else(|| "unknown".to_string());
                // Try compile-time record type first
                let is_record = self.type_defs.get(&type_name)
                    .map(|td| matches!(&td.kind, TypeDefKind::Record(_)))
                    .unwrap_or(false);
                if is_record {
                    let field_names: Vec<String> = self.type_defs.get(&type_name)
                        .map(|td| match &td.kind {
                            TypeDefKind::Record(fields) => fields.iter().map(|f| f.name.clone()).collect(),
                            _ => vec![],
                        })
                        .unwrap_or_default();
                    if name == "keys" {
                        return self.build_string_list(&field_names, vars);
                    } else {
                        // values: extract field values from record
                        let field_count = field_names.len();
                        let llvm_ty = self.type_llvm.get(&type_name).cloned();
                        if let Some(BasicTypeEnum::StructType(_struct_ty)) = llvm_ty {
                            let i64_ty = self.context.i64_type();
                            let sizeof_i64 = i64_ty.const_int(8, false);
                            let alloc_size = self.builder.build_int_mul(
                                i64_ty.const_int(field_count as u64, false),
                                sizeof_i64,
                                "values_alloc_size"
                            ).map_err(|e| CompileError::LlvmError(format!("mul error: {}", e)))?;
                            let malloc_fn = self.module.get_function("malloc")
                                .ok_or_else(|| "malloc not declared".to_string())?;
                            let values_data = self.builder.build_call(malloc_fn, &[
                                BasicMetadataValueEnum::IntValue(alloc_size),
                            ], "values_malloc")
                                .map_err(|e| CompileError::LlvmError(format!("malloc error: {}", e)))?
                                .try_as_basic_value_opt()
                                .ok_or("malloc returned void")?
                                .into_pointer_value();
                            let values_data_i64 = self.builder.build_bit_cast(values_data,
                                i64_ty.ptr_type(inkwell::AddressSpace::default()), "values_data_i64")
                                .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?
                                .into_pointer_value();
                            let record_ptr = match self.compile_expr(&args[0], vars)? {
                                BasicValueEnum::PointerValue(pv) => pv,
                                _ => return Err("values: expected record pointer".into()),
                            };
                            let type_def = self.type_defs.get(&type_name).ok_or_else(|| format!("values: unknown type '{}'", type_name))?;
                            if let TypeDefKind::Record(fields) = &type_def.kind {
                                for (i, field) in fields.iter().enumerate() {
                                    let gep = self.builder.build_struct_gep(_struct_ty, record_ptr, i as u32, &field.name)
                                        .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                                    let field_ty = types::mimi_type_to_llvm(self.context, &field.ty)
                                        .unwrap_or(BasicTypeEnum::IntType(i64_ty));
                                    let val = self.builder.build_load(field_ty, gep, &field.name)
                                        .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                                    let val_i64 = match val {
                                        BasicValueEnum::IntValue(iv) => iv,
                                        BasicValueEnum::FloatValue(fv) => self.builder.build_float_to_unsigned_int(fv, i64_ty, "float_to_i64")
                                            .map_err(|e| CompileError::LlvmError(format!("fptosi error: {}", e)))?,
                                        BasicValueEnum::PointerValue(pv) => self.builder.build_ptr_to_int(pv, i64_ty, "ptr_to_i64")
                                            .map_err(|e| CompileError::LlvmError(format!("ptrtoint error: {}", e)))?,
                                        _ => return Err("values: unsupported field type".into()),
                                    };
                                    // SAFETY: build_gep requires valid pointer and index types; the pointer is derived from a valid LLVM-typed allocation and indices are correctly-typed i64 values.
                                    // SAFETY: SAFETY: values_data_i64 is i64* from malloc; i is in-bounds (small constant index).
                                    let elem_ptr = unsafe { self.builder.build_gep(i64_ty, values_data_i64, &[i64_ty.const_int(i as u64, false)], "values_elem") }
                                        .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                                    self.builder.build_store(elem_ptr, val_i64)
                                        .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                                }
                                let result_list_ty = self.context.struct_type(&[
                                    BasicTypeEnum::IntType(i64_ty),
                                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                                ], false);
                                let result_alloca = self.builder.build_alloca(result_list_ty, "values_result")
                                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                                let result_len_gep = self.builder.build_struct_gep(result_list_ty, result_alloca, 0, "values_result_len")
                                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                                self.builder.build_store(result_len_gep, i64_ty.const_int(field_count as u64, false))
                                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                                let result_data_gep = self.builder.build_struct_gep(result_list_ty, result_alloca, 1, "values_result_data")
                                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                                let values_data_void = self.builder.build_bit_cast(values_data,
                                    self.context.ptr_type(inkwell::AddressSpace::default()), "values_data_void")
                                    .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?;
                                self.builder.build_store(result_data_gep, values_data_void)
                                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                                return Ok(result_alloca.into());
                            }
                        }
                    }
                }
                // Runtime map fallback: compile arg and call builtin
                let compiled_arg = self.compile_expr(&args[0], vars)?;
                let metadata_arg = match compiled_arg {
                    BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(iv),
                    BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(pv),
                    _ => return Err("keys/values: runtime fallback expects i64 or pointer".into()),
                };
                self.compile_builtin_call(name, &[metadata_arg]).map_err(|e| CompileError::Generic(e.to_string()))
            }
            // map/list, fn_ref): compile-time list iteration + function call
            "map" | "filter" if args.len() == 2 => {
                let is_map = name == "map";
                // Compile the list expression
                let list_val = self.compile_expr(&args[0], vars)?;
                let list_ptr = match list_val {
                    BasicValueEnum::PointerValue(pv) => pv,
                    _ => return Err("map/filter: first arg must be a list".into()),
                };
                // Resolve function name from second arg (must be an identifier)
                let fn_name = match &args[1] {
                    Expr::Ident(n) => n.clone(),
                    _ => return Err("map/filter: second arg must be a function name (identifier)".into()),
                };
                let fn_llvm = self.module.get_function(&fn_name)
                    .ok_or_else(|| format!("map/filter: function '{}' not compiled", fn_name))?;
                let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let i64_ty = self.context.i64_type();
                let list_struct_ty = BasicTypeEnum::StructType(self.context.struct_type(&[
                    BasicTypeEnum::IntType(i64_ty),
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                ], false));
                // Read list length and data pointer
                let len_gep = self.builder.build_struct_gep(list_struct_ty, list_ptr, 0, "len")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let list_len = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), len_gep, "len")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                let data_gep = self.builder.build_struct_gep(list_struct_ty, list_ptr, 1, "data")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let data_i8 = self.builder.build_load(BasicTypeEnum::PointerType(i8_ptr), data_gep, "data")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_pointer_value();
                let data_ptr = self.builder.build_bit_cast(data_i8,
                    i64_ty.ptr_type(inkwell::AddressSpace::default()), "data_i64")
                    .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?
                    .into_pointer_value();
                // Build result list: allocate {i64 len, i8* data}
                let result_ty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(i64_ty),
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                ], false);
                let result_alloca = self.builder.build_alloca(result_ty, "map_result")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                // Allocate output data array (same len)
                let elem_size = i64_ty.const_int(8, false);
                let alloc_size = self.builder.build_int_mul(list_len.into_int_value(), elem_size, "alloc_size")
                    .map_err(|e| CompileError::LlvmError(format!("mul error: {}", e)))?;
                let malloc_fn = self.module.get_function("malloc")
                    .ok_or_else(|| "malloc not declared".to_string())?;
                let out_ptr = self.builder.build_call(malloc_fn, &[
                    BasicMetadataValueEnum::IntValue(alloc_size),
                ], "out_malloc")
                    .map_err(|e| CompileError::LlvmError(format!("malloc error: {}", e)))?
                    .try_as_basic_value_opt()
                    .ok_or("malloc returned void")?
                    .into_pointer_value();
                let out_i64 = self.builder.build_bit_cast(out_ptr,
                    i64_ty.ptr_type(inkwell::AddressSpace::default()), "out_i64")
                    .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?
                    .into_pointer_value();
                // Loop: for i in 0..len
                let function = self.current_function().ok_or_else(|| "codegen: no current function for hof loop".to_string())?;
                let loop_bb = self.context.append_basic_block(function, "hof_loop");
                let body_bb = self.context.append_basic_block(function, "hof_body");
                let done_bb = self.context.append_basic_block(function, "hof_done");
                let idx_alloca = self.builder.build_alloca(i64_ty, "hi")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                let write_idx = self.builder.build_alloca(i64_ty, "wi")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(idx_alloca, i64_ty.const_int(0, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_store(write_idx, i64_ty.const_int(0, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(loop_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(loop_bb);
                let idx = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), idx_alloca, "idx")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_int_value();
                let loop_cmp = self.builder.build_int_compare(inkwell::IntPredicate::SLT, idx, list_len.into_int_value(), "cmp")
                    .map_err(|e| CompileError::LlvmError(format!("cmp error: {}", e)))?;
                self.builder.build_conditional_branch(loop_cmp, body_bb, done_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(body_bb);
                // Load element
                // SAFETY: build_gep requires valid pointer and index types; the pointer is derived from a valid LLVM-typed allocation and indices are correctly-typed i64 values.
                let elem_ptr = unsafe {
                    self.builder.build_gep(i64_ty, data_ptr, &[idx], "elem")
                }.map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let elem = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), elem_ptr, "elem_val")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                // Call the function: fn(elem)
                let fn_call = self.builder.build_call(fn_llvm, &[
                    BasicMetadataValueEnum::IntValue(elem.into_int_value()),
                ], "fn_call")
                    .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?;
                let result = call_try_basic_value(&fn_call)
                    .ok_or("function returned void")?;
                if is_map {
                    // For map: store result to output array
                    // SAFETY: build_gep requires valid pointer and index types; the pointer is derived from a valid LLVM-typed allocation and indices are correctly-typed i64 values.
                    let out_elem_ptr = unsafe {
                        self.builder.build_gep(i64_ty, out_i64, &[idx], "out_elem")
                    }.map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                    self.builder.build_store(out_elem_ptr, result)
                        .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                } else {
                    // For filter: if result is truthy (non-zero), store to output array
                    let zero = i64_ty.const_int(0, false);
                    // Zero-extend result to i64 for comparison (result may be i1 bool)
                    let result_i64 = self.builder.build_int_z_extend(result.into_int_value(), i64_ty, "result_ext")
                        .map_err(|e| CompileError::LlvmError(format!("zext error: {}", e)))?;
                    let truthy = self.builder.build_int_compare(inkwell::IntPredicate::NE, result_i64, zero, "truthy")
                        .map_err(|e| CompileError::LlvmError(format!("cmp error: {}", e)))?;
                    let store_bb = self.context.append_basic_block(function, "filter_store");
                    let next_bb = self.context.append_basic_block(function, "filter_next");
                    self.builder.build_conditional_branch(truthy, store_bb, next_bb)
                        .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                    self.builder.position_at_end(store_bb);
                    let wi = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), write_idx, "wi")
                        .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_int_value();
                    // SAFETY: build_gep requires valid pointer and index types; the pointer is derived from a valid LLVM-typed allocation and indices are correctly-typed i64 values.
                    let out_elem_ptr = unsafe {
                        self.builder.build_gep(i64_ty, out_i64, &[wi], "out_elem")
                    }.map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                    self.builder.build_store(out_elem_ptr, elem)
                        .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                    let next_wi = self.builder.build_int_add(wi, i64_ty.const_int(1, false), "next_wi")
                        .map_err(|e| CompileError::LlvmError(format!("add error: {}", e)))?;
                    self.builder.build_store(write_idx, next_wi)
                        .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                    self.builder.build_unconditional_branch(next_bb)
                        .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                    self.builder.position_at_end(next_bb);
                }
                // idx++
                let next = self.builder.build_int_add(idx, i64_ty.const_int(1, false), "next")
                    .map_err(|e| CompileError::LlvmError(format!("add error: {}", e)))?;
                self.builder.build_store(idx_alloca, next)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(loop_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(done_bb);
                // Store result list: len and data ptr
                let out_len = if is_map {
                    list_len
                } else {
                    self.builder.build_load(BasicTypeEnum::IntType(i64_ty), write_idx, "out_len")
                        .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?
                };
                let out_len_gep = self.builder.build_struct_gep(result_ty, result_alloca, 0, "out_len")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(out_len_gep, out_len)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let out_data_gep = self.builder.build_struct_gep(result_ty, result_alloca, 1, "out_data")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let out_void = self.builder.build_pointer_cast(out_i64, i8_ptr, "out_void")
                    .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?;
                self.builder.build_store(out_data_gep, out_void)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                Ok(result_alloca.into())
            }
            "reduce" if args.len() == 3 => {
                // reduce(list, fn, init) - function reference version
                let list_val = self.compile_expr(&args[0], vars)?;
                let list_ptr = match list_val {
                    BasicValueEnum::PointerValue(pv) => pv,
                    _ => return Err("reduce: first arg must be a list".into()),
                };
                let fn_name = match &args[1] {
                    Expr::Ident(n) => n.clone(),
                    _ => return Err("reduce: second arg must be a function name".into()),
                };
                let init_val = self.compile_expr(&args[2], vars)?;
                let fn_llvm = self.module.get_function(&fn_name)
                    .ok_or_else(|| format!("reduce: function '{}' not compiled", fn_name))?;
                let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let i64_ty = self.context.i64_type();
                let list_struct_ty = BasicTypeEnum::StructType(self.context.struct_type(&[
                    BasicTypeEnum::IntType(i64_ty),
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                ], false));
                let len_gep = self.builder.build_struct_gep(list_struct_ty, list_ptr, 0, "len")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let list_len = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), len_gep, "len")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                let data_gep = self.builder.build_struct_gep(list_struct_ty, list_ptr, 1, "data")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let data_i8 = self.builder.build_load(BasicTypeEnum::PointerType(i8_ptr), data_gep, "data")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_pointer_value();
                let data_ptr = self.builder.build_bit_cast(data_i8,
                    i64_ty.ptr_type(inkwell::AddressSpace::default()), "data_i64")
                    .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?
                    .into_pointer_value();
                let acc_alloca = self.builder.build_alloca(i64_ty, "acc")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(acc_alloca, init_val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let function = self.current_function().ok_or_else(|| "codegen: no current function for reduce loop".to_string())?;
                let loop_bb = self.context.append_basic_block(function, "reduce_loop");
                let body_bb = self.context.append_basic_block(function, "reduce_body");
                let done_bb = self.context.append_basic_block(function, "reduce_done");
                let idx_alloca = self.builder.build_alloca(i64_ty, "ri")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(idx_alloca, i64_ty.const_int(0, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(loop_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(loop_bb);
                let idx = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), idx_alloca, "idx")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_int_value();
                let loop_cmp = self.builder.build_int_compare(inkwell::IntPredicate::SLT, idx, list_len.into_int_value(), "cmp")
                    .map_err(|e| CompileError::LlvmError(format!("cmp error: {}", e)))?;
                self.builder.build_conditional_branch(loop_cmp, body_bb, done_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(body_bb);
                // SAFETY: build_gep requires valid pointer and index types; the pointer is derived from a valid LLVM-typed allocation and indices are correctly-typed i64 values.
                let elem_ptr = unsafe {
                    self.builder.build_gep(i64_ty, data_ptr, &[idx], "elem")
                }.map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let elem = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), elem_ptr, "elem_val")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                let acc = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), acc_alloca, "acc")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                let fn_result = self.builder.build_call(fn_llvm, &[
                    BasicMetadataValueEnum::IntValue(acc.into_int_value()),
                    BasicMetadataValueEnum::IntValue(elem.into_int_value()),
                ], "reduce_call")
                    .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?
                    .try_as_basic_value_opt()
                    .ok_or("function returned void")?;
                self.builder.build_store(acc_alloca, fn_result)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let next = self.builder.build_int_add(idx, i64_ty.const_int(1, false), "next")
                    .map_err(|e| CompileError::LlvmError(format!("add error: {}", e)))?;
                self.builder.build_store(idx_alloca, next)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(loop_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(done_bb);
                let result = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), acc_alloca, "result")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                Ok(result)
            }
            _ => Err(format!("unknown compile-time builtin '{}'", name).into()),
        }
    }


    pub(in crate::codegen) fn compile_call_expr(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        match callee {
            Expr::Ident(name) => {
                match name.as_str() {
                    "type_name" | "type_fields" | "type_variants" | "keys" | "values"
                    | "map" | "filter" | "reduce" => {
                        return self.compile_builtin_intrinsic(name, args, vars);
                    }
                    _ => {}
                }
                // Check if this is a closure variable call
                if let Some(&(alloca, ty)) = vars.get(name.as_str()) {
                    if let BasicTypeEnum::StructType(st) = ty {
                        if st.get_field_types().len() == 2 {
                            // Closure struct {fn_ptr, env_ptr} — do indirect call
                            let closure_val = self.builder.build_load(
                                BasicTypeEnum::StructType(st), alloca,
                                &format!("{}_closure", name),
                            ).map_err(|e| CompileError::LlvmError(format!("load closure error: {}", e)))?;
                            let closure_struct = closure_val.into_struct_value();
                            let fn_ptr = self.builder.build_extract_value(closure_struct, 0, "fn_ptr")
                                .map_err(|e| CompileError::LlvmError(format!("extract fn_ptr error: {}", e)))?
                                .into_pointer_value();
                            let env_ptr = self.builder.build_extract_value(closure_struct, 1, "env_ptr")
                                .map_err(|e| CompileError::LlvmError(format!("extract env_ptr error: {}", e)))?
                                .into_pointer_value();
                            let mut compiled_args = Vec::new();
                            for arg in args {
                                compiled_args.push(self.compile_expr(arg, vars)?);
                            }
                            let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                            let env_meta = BasicMetadataTypeEnum::PointerType(i8_ptr);
                            let mut all_meta = vec![env_meta];
                            for arg in &compiled_args {
                                all_meta.push(match arg {
                                    BasicValueEnum::IntValue(iv) => BasicMetadataTypeEnum::IntType(iv.get_type()),
                                    BasicValueEnum::FloatValue(fv) => BasicMetadataTypeEnum::FloatType(fv.get_type()),
                                    BasicValueEnum::PointerValue(pv) => BasicMetadataTypeEnum::PointerType(pv.get_type()),
                                    BasicValueEnum::StructValue(sv) => BasicMetadataTypeEnum::StructType(sv.get_type()),
                                    BasicValueEnum::ArrayValue(av) => BasicMetadataTypeEnum::ArrayType(av.get_type()),
                                    BasicValueEnum::VectorValue(vv) => BasicMetadataTypeEnum::VectorType(vv.get_type()),
                                    BasicValueEnum::ScalableVectorValue(_) => BasicMetadataTypeEnum::IntType(self.context.i64_type()),
                                });
                            }
                            let ret_type = self.context.i64_type();
                            let indirect_fn_type = ret_type.fn_type(&all_meta, false);
                            let fn_ptr_typed = self.builder.build_pointer_cast(
                                fn_ptr,
                                indirect_fn_type.ptr_type(inkwell::AddressSpace::default()),
                                "fn_typed",
                            ).map_err(|e| CompileError::LlvmError(format!("pointer cast error: {}", e)))?;
                            let mut call_args = vec![BasicMetadataValueEnum::PointerValue(env_ptr)];
                            for arg in &compiled_args {
                                call_args.push(match arg {
                                    BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                                    BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                                    BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                                    BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                                    BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                                    BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                                    BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
                                });
                            }
                            let call = self.builder.build_indirect_call(
                                indirect_fn_type, fn_ptr_typed, &call_args, "closure_call",
                            ).map_err(|e| CompileError::LlvmError(format!("closure call error: {}", e)))?;
                            return Ok(call_try_basic_value(&call).unwrap_or(
                                self.context.i64_type().const_int(0, false).into()
                            ));
                        }
                    }
                }
                self.compile_call(name, args, vars)
            }
            Expr::Field(obj, method_name) => {
                self.compile_method_call(obj, method_name, args, vars)
            }
            _ => Err("only direct function calls and method calls supported in codegen".into()),
        }
    }


    /// Handle method dispatch for obj.method(args) calls.
    pub(in crate::codegen) fn compile_method_call(
        &mut self,
        obj: &Expr,
        method_name: &str,
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        // Method call: obj.method(args)
        // Determine the type of the object to find the actor/trait name
        let obj_type = self.infer_object_type(obj, vars);
        let actor_method = format!("{}__{}__method", obj_type, method_name);
        
        // 1. Try actor method dispatch
        if let Some(function) = self.module.get_function(&actor_method) {
            let mut obj_val = self.compile_expr(obj, vars)?;
            // Actor methods take self as pointer; convert struct value to pointer if needed
            if let BasicValueEnum::StructValue(sv) = obj_val {
                let struct_ty = sv.get_type();
                let alloca = self.builder.build_alloca(struct_ty, "self_tmp")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(alloca, obj_val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                obj_val = alloca.into();
            }
            let mut compiled_args = Vec::new();
            compiled_args.push(obj_val);
            for arg in args {
                compiled_args.push(self.compile_expr(arg, vars)?);
            }
            let metadata_args: Vec<_> = compiled_args.iter().map(|v| match v {
                BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                    BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
            }).collect();
            let call = self.builder.build_call(function, &metadata_args, "method_call")
                .map_err(|e| CompileError::LlvmError(format!("method call error: {}", e)))?;
            return Ok(call_try_basic_value(&call).unwrap_or(
                self.context.i64_type().const_int(0, false).into()
            ));
        }

        // 1.2. Variant method dispatch (Result/Option combinators)
        if obj_type.starts_with("Result<") || obj_type.starts_with("Option<")
            || obj_type == "Result" || obj_type == "Option" {
            if let Ok(result) = self.compile_variant_method(obj, method_name, args, vars) {
                return Ok(result);
            }
        }

        // 1.5. Special case: Type.spawn() constructor call for actors
        if method_name == "spawn" {
            let spawn_name = format!("{}_spawn", obj_type);
            if let Some(spawn_fn) = self.module.get_function(&spawn_name) {
                let call = self.builder.build_call(spawn_fn, &[], "actor_spawn")
                    .map_err(|e| CompileError::LlvmError(format!("spawn call error: {}", e)))?;
                return Ok(call_try_basic_value(&call).unwrap_or(
                    self.context.i64_type().const_int(0, false).into()
                ));
            }
        }

        // 2. Try trait method dispatch: type_impls[type_name][trait_name][method_name]
        if let Some(trait_impls) = self.type_impls.get(&obj_type) {
            for (trait_name, methods) in trait_impls {
                if methods.iter().any(|m| m.name == *method_name) {
                    let mangled = format!("{}__{}__{}", obj_type, trait_name, method_name);
                    if let Some(function) = self.module.get_function(&mangled) {
                        let obj_val = self.compile_expr(obj, vars)?;
                        let obj_val = match obj_val {
                            BasicValueEnum::StructValue(sv) => {
                                let struct_ty = sv.get_type();
                                let alloca = self.builder.build_alloca(struct_ty, "self_tmp")
                                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                                self.builder.build_store(alloca, sv)
                                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                                BasicValueEnum::PointerValue(alloca)
                            }
                            other => other,
                        };
                        let mut compiled_args = Vec::new();
                        compiled_args.push(obj_val);
                        for arg in args {
                            compiled_args.push(self.compile_expr(arg, vars)?);
                        }
                        let metadata_args: Vec<_> = compiled_args.iter().map(|v| match v {
                            BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                            BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                            BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                            BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                            BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                            BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                            BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
                        }).collect();
                        let call = self.builder.build_call(function, &metadata_args, "trait_call")
                            .map_err(|e| CompileError::LlvmError(format!("trait method call error: {}", e)))?;
                        return Ok(call_try_basic_value(&call).unwrap_or(
                            self.context.i64_type().const_int(0, false).into()
                        ));
                    }
                }
            }
        }
        // 3. True vtable indirect dispatch for dyn Trait objects
        if obj_type.starts_with("dyn ") {
            let trait_name = obj_type.strip_prefix("dyn ").unwrap_or("");
            if !trait_name.is_empty() && !trait_name.contains(' ') {
                // Find method index within the trait definition
                let method_idx = self.trait_defs.get(trait_name)
                    .and_then(|tdef| tdef.methods.iter().position(|m| m.name == *method_name));
                if let Some(idx) = method_idx {
                    // Get the vtable struct type (clone to avoid borrow conflict)
                    let vtable_ty = self.vtable_types.get(trait_name)
                        .map(|s| *s).ok_or("no vtable type for trait")?;
                    // Fat pointer layout: { i8* data, i8* vtable }
                    let i8_ptr_ty = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                    let fat_ty = self.context.struct_type(&[
                        BasicTypeEnum::PointerType(i8_ptr_ty),
                        BasicTypeEnum::PointerType(i8_ptr_ty),
                    ], false);
                    // The obj_val is a fat pointer struct { data: i8*, vtable: i8* }
                    let obj_val = self.compile_expr(obj, vars)?;
                    let fat_ptr = match obj_val {
                            BasicValueEnum::StructValue(_) => {
                                // Alloca the struct value so we can GEP into it
                                let alloca = self.builder.build_alloca(
                                    BasicTypeEnum::StructType(fat_ty), "fat_tmp"
                                ).map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                                self.builder.build_store(alloca, obj_val)
                                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                                alloca
                            }
                            BasicValueEnum::PointerValue(pv) => pv,
                            _ => return Err("dyn Trait value must be a struct or pointer".into()),
                        };
                        // Extract vtable pointer (field 1)
                        let vtable_gep = self.builder.build_struct_gep(
                            BasicTypeEnum::StructType(fat_ty), fat_ptr, 1, "vtable_gep"
                        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                        let vtable_ptr = self.builder.build_load(
                            BasicTypeEnum::PointerType(i8_ptr_ty), vtable_gep, "vtable_ptr"
                        ).map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_pointer_value();
                        // GEP into vtable at method index
                        let method_gep = self.builder.build_struct_gep(
                            BasicTypeEnum::StructType(vtable_ty), vtable_ptr, idx as u32, "method_gep"
                        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                        // Load function pointer from vtable slot
                        let fn_ptr = self.builder.build_load(
                            BasicTypeEnum::PointerType(i8_ptr_ty), method_gep, "fn_ptr"
                        ).map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_pointer_value();
                        // Extract data pointer (field 0) for passing as self arg
                        let data_gep = self.builder.build_struct_gep(
                            BasicTypeEnum::StructType(fat_ty), fat_ptr, 0, "data_gep"
                        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                        let data_ptr = self.builder.build_load(
                            BasicTypeEnum::PointerType(i8_ptr_ty), data_gep, "data_ptr"
                        ).map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                        // Get the mangled function's type for the indirect call signature
                        // Find any matching mangled function to extract fn type
                        let fn_sig = (|| -> Option<(inkwell::values::AnyValueEnum<'ctx>, String)> {
                            for (tn, timpls) in &self.type_impls {
                                if let Some(methods) = timpls.get(trait_name) {
                                    if methods.iter().any(|m| m.name == *method_name) {
                                        let mangled = format!("{}__{}__{}", tn, trait_name, method_name);
                                        if let Some(f) = self.module.get_function(&mangled) {
                                            return Some((inkwell::values::AnyValueEnum::FunctionValue(f), mangled));
                                        }
                                    }
                                }
                            }
                            None
                        })();
                        if let Some((fn_val, _)) = fn_sig {
                            let fn_llvm = fn_val.into_function_value();
                            let fn_type = fn_llvm.get_type();
                            // Cast fn_ptr i8* to the right function pointer type
                            let fn_ptr_cast = self.builder.build_pointer_cast(
                                fn_ptr,
                                fn_type.ptr_type(inkwell::AddressSpace::default()),
                                "fn_cast"
                            ).map_err(|e| CompileError::LlvmError(format!("cast error: {}", e)))?;
                            // Compile additional args (start with data ptr as self)
                            let mut compiled_args = Vec::new();
                            compiled_args.push(data_ptr);
                            for arg in args {
                                compiled_args.push(self.compile_expr(arg, vars)?);
                            }
                            let metadata_args: Vec<_> = compiled_args.iter().map(|v| match v {
                                BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                                BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                                BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                                BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                                BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                                BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                                BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
                            }).collect();
                            let call = self.builder.build_indirect_call(
                                fn_type, fn_ptr_cast, &metadata_args, "dyn_call"
                            ).map_err(|e| CompileError::LlvmError(format!("dyn indirect call error: {}", e)))?;
                            return Ok(call_try_basic_value(&call).unwrap_or(
                                self.context.i64_type().const_int(0, false).into()
                            ));
                        }
                }
            }
            return Err(format!("[E0708] cannot dispatch method '{}' on {}", method_name, obj_type).into());
        }

        // 3b. Try impl Trait dispatch (same logic as dyn Trait)
        if obj_type.starts_with("impl ") {
            let trait_name = obj_type.strip_prefix("impl ").unwrap_or("");
            if !trait_name.is_empty() && !trait_name.contains(' ') {
                for (type_name, trait_impls) in &self.type_impls {
                    if let Some(methods) = trait_impls.get(trait_name) {
                        if methods.iter().any(|m| m.name == *method_name) {
                            let mangled = format!("{}__{}__{}", type_name, trait_name, method_name);
                            if let Some(function) = self.module.get_function(&mangled) {
                                let obj_val = self.compile_expr(obj, vars)?;
                                let obj_val = match obj_val {
                                    BasicValueEnum::StructValue(sv) => {
                                        let struct_ty = sv.get_type();
                                        let alloca = self.builder.build_alloca(struct_ty, "self_tmp")
                                            .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                                        self.builder.build_store(alloca, sv)
                                            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                                        BasicValueEnum::PointerValue(alloca)
                                    }
                                    other => other,
                                };
                                let mut compiled_args = Vec::new();
                                compiled_args.push(obj_val);
                                for arg in args {
                                    compiled_args.push(self.compile_expr(arg, vars)?);
                                }
                                let metadata_args: Vec<_> = compiled_args.iter().map(|v| match v {
                                    BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                                    BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                                    BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                                    BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                                    BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                                    BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                                    BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
                                }).collect();
                                let call = self.builder.build_call(function, &metadata_args, "impl_trait_call")
                                    .map_err(|e| CompileError::LlvmError(format!("impl trait call error: {}", e)))?;
                                return Ok(call_try_basic_value(&call).unwrap_or(
                                    self.context.i64_type().const_int(0, false).into()
                                ));
                            }
                        }
                    }
                }
            }
            return Err(format!("[E0708] cannot dispatch method '{}' on {}", method_name, obj_type).into());
        }

        // 4. Try enum constructor: {Type}_{Variant}(args)
        if self.type_defs.contains_key(&obj_type) {
            let ctor_name = format!("{}_{}", obj_type, method_name);
            if let Some(function) = self.module.get_function(&ctor_name) {
                let mut compiled_args = Vec::new();
                for arg in args {
                    compiled_args.push(self.compile_expr(arg, vars)?);
                }
                let metadata_args: Vec<_> = compiled_args.iter().map(|v| match v {
                    BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                    BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                    BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                    BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                    BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                    BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                    BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
                }).collect();
                let call = self.builder.build_call(function, &metadata_args, "enum_ctor")
                    .map_err(|e| CompileError::LlvmError(format!("enum ctor call error: {}", e)))?;
                return Ok(call_try_basic_value(&call).unwrap_or(
                    self.context.i64_type().const_int(0, false).into()
                ));
            }
            Err(CompileError::Generic(format!("method '{}' not compiled for type '{}' (missing crate?)", method_name, obj_type)))
        } else {
            Err(format!("cannot call method '{}' on unknown type '{}'", method_name, obj_type).into())
        }
    }


    pub(in crate::codegen) fn compile_turbofish_expr(
        &mut self,
        name: &str,
        type_args: &[Type],
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        // Monomorphized call: func::<Type>(args)
        // Build type_map from explicit type args
        let func = self.find_func_def(name)?;
        if func.generics.len() != type_args.len() {
            return Err(CompileError::Generic(format!("[E0720] turbofish for '{}' expects {} type args, got {}", name, func.generics.len(), type_args.len())));
        }
        let mut turbo_map: HashMap<String, crate::ast::Type> = HashMap::new();
        for (gp, ta) in func.generics.iter().zip(type_args.iter()) {
            turbo_map.insert(gp.name.clone(), ta.clone());
        }
        // Merge with current type_map (for nested generics)
        let mut merged_map = self.type_map.clone();
        merged_map.extend(turbo_map);
        let mangled = Self::mangle_name(name, &merged_map);
        // Compile the specialized version if not yet compiled
        if self.module.get_function(&mangled).is_none() {
            self.compile_generic_func(&func, &merged_map).map_err(|e| CompileError::Generic(e.to_string()))?;
        }
        // Call the mangled function
        self.compile_call_mangled(&mangled, args, vars)
    }


    /// Copy the error variant from source to result (for Result types),
    /// writing {disc=0, ok=0, err=<copied>} into result_alloca.
    pub(in crate::codegen) fn emit_variant_err_path(
        &self,
        is_result: bool,
        variant_sty: inkwell::types::StructType<'ctx>,
        pv: inkwell::values::PointerValue<'ctx>,
        result_alloca: inkwell::values::PointerValue<'ctx>,
    ) -> Result<(), CompileError> {
        let i1_ty = self.context.bool_type();
        let i64_ty = self.context.i64_type();
        let d_gep_e = self.builder.build_struct_gep(
            BasicTypeEnum::StructType(variant_sty), result_alloca, 0, "d_gep_e"
        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
        self.builder.build_store(d_gep_e, i1_ty.const_int(0, false))
            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        let o_gep_e = self.builder.build_struct_gep(
            BasicTypeEnum::StructType(variant_sty), result_alloca, 1, "o_gep_e"
        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
        self.builder.build_store(o_gep_e, i64_ty.const_int(0, false))
            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        if is_result {
            let src_err_gep = self.builder.build_struct_gep(
                BasicTypeEnum::StructType(variant_sty), pv, 2, "src_err_gep"
            ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
            let err_val = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), src_err_gep, "err_val")
                .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
            let dst_err_gep = self.builder.build_struct_gep(
                BasicTypeEnum::StructType(variant_sty), result_alloca, 2, "dst_err_gep"
            ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
            self.builder.build_store(dst_err_gep, err_val)
                .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        }
        Ok(())
    }


    pub(in crate::codegen) fn compile_variant_method(
        &mut self,
        obj: &Expr,
        method: &str,
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        let obj_val = self.compile_expr(obj, vars)?;
        let obj_type = self.infer_object_type(obj, vars);
        let is_result = obj_type.starts_with("Result<") || obj_type == "Result";
        let i1_ty = self.context.bool_type();
        let i64_ty = self.context.i64_type();
        let function = self.current_function().ok_or_else(|| "codegen: no current function for variant method".to_string())?;

        // Layout: Result<T,E> = {i1 disc, T ok, i64 err}, Option<T> = {i1 disc, T payload}
        let disc_idx: u32 = 0;
        let payload_idx: u32 = 1;
        let variant_sty = if is_result {
            self.context.struct_type(&[
                BasicTypeEnum::IntType(i1_ty),
                BasicTypeEnum::IntType(i64_ty),
                BasicTypeEnum::IntType(i64_ty),
            ], false)
        } else {
            self.context.struct_type(&[
                BasicTypeEnum::IntType(i1_ty),
                BasicTypeEnum::IntType(i64_ty),
            ], false)
        };

        // Convert StructValue to PointerValue for uniform handling,
        // and determine the actual struct type for correct GEP offsets.
        // The actual struct layout depends on the payload type T,
        // e.g. {i1, i32, i64} for Result<i32,string> vs {i1, i64, i64} for Result<i64,string>.
        let (pv, actual_sty_enum) = match obj_val {
            BasicValueEnum::PointerValue(pv) => {
                let sty = if let Expr::Ident(name) = obj {
                    vars.get(name.as_str())
                        .map(|entry| entry.1)
                        .unwrap_or(BasicTypeEnum::StructType(variant_sty))
                } else {
                    BasicTypeEnum::StructType(variant_sty)
                };
                (pv, sty)
            }
            BasicValueEnum::StructValue(sv) => {
                let sty = sv.get_type();
                let sty_enum = BasicTypeEnum::StructType(sty);
                let tmp = self.builder.build_alloca(sty_enum, "variant_tmp")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(tmp, sv)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                (tmp, sty_enum)
            }
            _ => return Err(format!("variant method '{}' requires a struct pointer or value", method).into()),
        };
        let disc_gep = self.builder.build_struct_gep(
            actual_sty_enum, pv, disc_idx, "disc_gep"
        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
        let disc = self.builder.build_load(BasicTypeEnum::IntType(i1_ty), disc_gep, "disc")
            .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_int_value();
        let pay_gep = self.builder.build_struct_gep(
            actual_sty_enum, pv, payload_idx, "pay_gep"
        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
        let payload = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), pay_gep, "payload")
            .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;

        match method {
            "is_ok" | "is_some" => {
                let bool_val = self.builder.build_int_z_extend(disc, self.context.bool_type(), "is_ok_ext")
                    .map_err(|e| CompileError::LlvmError(format!("zext error: {}", e)))?;
                Ok(BasicValueEnum::IntValue(bool_val))
            }
            "is_err" | "is_none" => {
                let not_disc = self.builder.build_not(disc, "is_err_not")
                    .map_err(|e| CompileError::LlvmError(format!("not error: {}", e)))?;
                let bool_val = self.builder.build_int_z_extend(not_disc, self.context.bool_type(), "is_err_ext")
                    .map_err(|e| CompileError::LlvmError(format!("zext error: {}", e)))?;
                Ok(BasicValueEnum::IntValue(bool_val))
            }
            "unwrap" | "expect" => {
                let ok_bb = self.context.append_basic_block(function, "unwrap_ok");
                let err_bb = self.context.append_basic_block(function, "unwrap_err");
                self.builder.build_conditional_branch(disc, ok_bb, err_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(err_bb);
                let trap_fn = self.module.get_function("mimi_try_exit")
                    .or_else(|| self.module.get_function("abort"))
                    .ok_or("abort not declared")?;
                self.builder.build_call(trap_fn, &[
                    BasicMetadataValueEnum::IntValue(payload.into_int_value()),
                ], "unwrap_trap").map_err(|e| CompileError::LlvmError(format!("trap error: {}", e)))?;
                let unreachable = self.context.append_basic_block(function, "unreachable");
                self.builder.build_unconditional_branch(unreachable)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(unreachable);
                self.builder.build_unreachable()
                    .map_err(|e| CompileError::LlvmError(format!("unreachable terminator: {}", e)))?;
                self.builder.position_at_end(ok_bb);
                Ok(payload)
            }
            "unwrap_or" => {
                if args.is_empty() {
                    return Err("unwrap_or requires a default value".into());
                }
                let default_val = self.compile_expr(&args[0], vars)?;
                let ok_bb = self.context.append_basic_block(function, "unwrap_or_ok");
                let done_bb = self.context.append_basic_block(function, "unwrap_or_done");
                let result_alloca = self.builder.build_alloca(BasicTypeEnum::IntType(i64_ty), "unwrap_or_result")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(result_alloca, payload)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_conditional_branch(disc, ok_bb, done_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(done_bb);
                self.builder.build_store(result_alloca, default_val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(ok_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(ok_bb);
                self.builder.build_load(BasicTypeEnum::IntType(i64_ty), result_alloca, "unwrap_or_val")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))
            }
            "ok_or" => {
                if args.is_empty() {
                    return Err("ok_or requires an error value".into());
                }
                let err_val = self.compile_expr(&args[0], vars)?;
                let ok_bb = self.context.append_basic_block(function, "ok_or_ok");
                let done_bb = self.context.append_basic_block(function, "ok_or_done");
                let result_sty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(i1_ty),
                    BasicTypeEnum::IntType(i64_ty),
                    BasicTypeEnum::IntType(i64_ty),
                ], false);
                let result_alloca = self.builder.build_alloca(BasicTypeEnum::StructType(result_sty), "ok_or_result")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                let disc_gep = self.builder.build_struct_gep(
                    BasicTypeEnum::StructType(result_sty), result_alloca, 0, "disc_gep"
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(disc_gep, self.context.bool_type().const_int(1, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let ok_gep = self.builder.build_struct_gep(
                    BasicTypeEnum::StructType(result_sty), result_alloca, 1, "ok_gep"
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(ok_gep, payload)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(ok_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(done_bb);
                let disc_gep2 = self.builder.build_struct_gep(
                    BasicTypeEnum::StructType(result_sty), result_alloca, 0, "disc_gep2"
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(disc_gep2, self.context.bool_type().const_int(0, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let err_gep = self.builder.build_struct_gep(
                    BasicTypeEnum::StructType(result_sty), result_alloca, 2, "err_gep"
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(err_gep, err_val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(ok_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(ok_bb);
                self.builder.build_load(BasicTypeEnum::StructType(result_sty), result_alloca, "ok_or_val")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))
            }
            "map" => {
                if args.is_empty() {
                    return Err("map requires a function argument".into());
                }
                let closure_val = self.compile_expr_or_func_ref(&args[0], vars)?;
                let ok_bb = self.context.append_basic_block(function, "variant_map_ok");
                let err_bb = self.context.append_basic_block(function, "variant_map_err");
                let merge_bb = self.context.append_basic_block(function, "variant_map_merge");
                let result_alloca = self.builder.build_alloca(
                    BasicTypeEnum::StructType(variant_sty), "variant_map_result"
                ).map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_conditional_branch(disc, ok_bb, err_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                // Err path: write Err variant {disc=0, ok=0, err=copy_from_source}
                self.builder.position_at_end(err_bb);
                self.emit_variant_err_path(is_result, variant_sty, pv, result_alloca)?;
                self.builder.build_unconditional_branch(merge_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                // Ok path: call fn(payload), write Ok variant {disc=1, ok=mapped}
                self.builder.position_at_end(ok_bb);
                let mapped = self.compile_call_fn_ref(closure_val, &args[0], payload, i64_ty)?;
                let d_gep_o = self.builder.build_struct_gep(
                    BasicTypeEnum::StructType(variant_sty), result_alloca, 0, "d_gep_o"
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(d_gep_o, self.context.bool_type().const_int(1, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let o_gep_o = self.builder.build_struct_gep(
                    BasicTypeEnum::StructType(variant_sty), result_alloca, 1, "o_gep_o"
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(o_gep_o, mapped)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(merge_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(merge_bb);
                self.builder.build_load(BasicTypeEnum::StructType(variant_sty), result_alloca, "variant_map_val")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))
            }
            "and_then" => {
                if args.is_empty() {
                    return Err("and_then requires a function argument".into());
                }
                let closure_val = self.compile_expr_or_func_ref(&args[0], vars)?;
                let ok_bb = self.context.append_basic_block(function, "variant_and_then_ok");
                let err_bb = self.context.append_basic_block(function, "variant_and_then_err");
                let merge_bb = self.context.append_basic_block(function, "variant_and_then_merge");
                let result_alloca = self.builder.build_alloca(
                    BasicTypeEnum::StructType(variant_sty), "variant_and_then_result"
                ).map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_conditional_branch(disc, ok_bb, err_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                // Err path: write Err variant {disc=0, ok=0, err=copy_from_source}
                self.builder.position_at_end(err_bb);
                self.emit_variant_err_path(is_result, variant_sty, pv, result_alloca)?;
                self.builder.build_unconditional_branch(merge_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                // Ok path: call fn(payload), store resulting variant into result_alloca
                self.builder.position_at_end(ok_bb);
                let fn_result = self.compile_call_fn_ref(closure_val, &args[0], payload, i64_ty)?;
                match fn_result {
                    BasicValueEnum::StructValue(sv) => {
                        self.builder.build_store(result_alloca, sv)
                            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                    }
                    _ => return Err("and_then: function must return a variant struct".into()),
                }
                self.builder.build_unconditional_branch(merge_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(merge_bb);
                self.builder.build_load(BasicTypeEnum::StructType(variant_sty), result_alloca, "variant_and_then_val")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))
            }
            "map_err" => {
                if args.is_empty() {
                    return Err("map_err requires a function argument".into());
                }
                if !is_result {
                    return Err("map_err is only available on Result types".into());
                }
                let closure_val = self.compile_expr_or_func_ref(&args[0], vars)?;
                let ok_bb = self.context.append_basic_block(function, "map_err_ok");
                let done_bb = self.context.append_basic_block(function, "map_err_done");
                let result_alloca = self.builder.build_alloca(BasicTypeEnum::IntType(i64_ty), "map_err_result")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(result_alloca, payload)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_conditional_branch(disc, ok_bb, done_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(done_bb);
                let err_gep = self.builder.build_struct_gep(
                    BasicTypeEnum::StructType(variant_sty), pv, 2, "err_gep"
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let err_payload = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), err_gep, "err_payload")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                let mapped = self.compile_closure_call(closure_val, err_payload.into_int_value())?;
                self.builder.build_store(result_alloca, mapped)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                self.builder.build_unconditional_branch(ok_bb)
                    .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                self.builder.position_at_end(ok_bb);
                self.builder.build_load(BasicTypeEnum::IntType(i64_ty), result_alloca, "map_err_val")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))
            }
            _ => Err(format!("variant '{}' has no method '{}'", obj_type, method).into()),
        }
    }


    /// Call a closure value (struct {fn_ptr, env_ptr}) with a single i64 argument,
    /// returning the i64 result.
    pub(in crate::codegen) fn compile_closure_call(
        &self,
        closure_val: BasicValueEnum<'ctx>,
        arg: inkwell::values::IntValue<'ctx>,
    ) -> Result<inkwell::values::BasicValueEnum<'ctx>, CompileError> {
        let i64_ty = self.context.i64_type();
        let (fn_ptr, env_ptr) = match closure_val {
            BasicValueEnum::StructValue(sv) => {
                let fn_ptr = self.builder.build_extract_value(sv, 0, "fn_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract fn_ptr error: {}", e)))?.into_pointer_value();
                let env_ptr = self.builder.build_extract_value(sv, 1, "env_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract env_ptr error: {}", e)))?.into_pointer_value();
                (fn_ptr, env_ptr)
            }
            BasicValueEnum::PointerValue(pv) => {
                let closure_struct_ty = self.context.struct_type(&[
                    BasicTypeEnum::PointerType(self.context.i8_type().ptr_type(inkwell::AddressSpace::default())),
                    BasicTypeEnum::PointerType(self.context.i8_type().ptr_type(inkwell::AddressSpace::default())),
                ], false);
                let loaded = self.builder.build_load(BasicTypeEnum::StructType(closure_struct_ty), pv, "closure_loaded")
                    .map_err(|e| CompileError::LlvmError(format!("load closure error: {}", e)))?.into_struct_value();
                let fn_ptr = self.builder.build_extract_value(loaded, 0, "fn_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract fn_ptr error: {}", e)))?.into_pointer_value();
                let env_ptr = self.builder.build_extract_value(loaded, 1, "env_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract env_ptr error: {}", e)))?.into_pointer_value();
                (fn_ptr, env_ptr)
            }
            _ => return Err(CompileError::Generic("expected a closure".into())),
        };
        let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
        let fn_type = i64_ty.fn_type(&[
            BasicMetadataTypeEnum::PointerType(i8_ptr),
            BasicMetadataTypeEnum::IntType(i64_ty),
        ], false);
        let fn_typed = self.builder.build_pointer_cast(
            fn_ptr, fn_type.ptr_type(inkwell::AddressSpace::default()), "fn_typed"
        ).map_err(|e| CompileError::LlvmError(format!("pointer cast error: {}", e)))?;
        let call = self.builder.build_indirect_call(
            fn_type, fn_typed, &[
                BasicMetadataValueEnum::PointerValue(env_ptr),
                BasicMetadataValueEnum::IntValue(arg),
            ], "closure_call"
        ).map_err(|e| CompileError::LlvmError(format!("indirect call error: {}", e)))?;
        let result = call_try_basic_value(&call)
            .unwrap_or(BasicValueEnum::IntValue(i64_ty.const_int(0, false)));
        Ok(result)
    }


    pub(in crate::codegen) fn compile_spawn_expr(
        &mut self,
        expr: &Expr,
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        // Spawn: create a thread to execute the expression
        let parent_fn = self.current_function().ok_or_else(|| "codegen: no current function for spawn".to_string())?;
        let parent_name = parent_fn.get_name().to_str().unwrap_or("unknown").to_string();
        let wrapper_name = format!("{}{}__spawn_wrapper", parent_name, self.spawn_counter).to_string();
        self.spawn_counter += 1;
        
        // Collect free variables from the spawn expression (capture by value)
        let mut free_vars: BTreeMap<String, (inkwell::values::PointerValue<'ctx>, BasicTypeEnum<'ctx>)> = BTreeMap::new();
        let empty_defined = std::collections::HashSet::new();
        self.collect_free_vars_expr(expr, &empty_defined, vars, &mut free_vars);
        
        // Create wrapper function: i8* wrapper(i8*)
        let i8_ty = self.context.i8_type();
        let i8_ptr = i8_ty.ptr_type(inkwell::AddressSpace::default());
        let wrapper_fn_type = i8_ptr.fn_type(
            &[BasicMetadataTypeEnum::PointerType(i8_ptr)], false
        );
        let wrapper_fn = self.module.add_function(&wrapper_name, wrapper_fn_type, None);
        let wrapper_entry = self.context.append_basic_block(wrapper_fn, "entry");
        
        // Save current builder position
        let saved_block = self.builder.get_insert_block();
        self.builder.position_at_end(wrapper_entry);
        
        // Build wrapper_vars: load captured variables from env_ptr arg
        let env_ptr_param = wrapper_fn.get_nth_param(0)
            .ok_or_else(|| "codegen: spawn wrapper env_ptr param index out of range".to_string())?
            .into_pointer_value();
        let mut wrapper_vars = HashMap::new();
        if !free_vars.is_empty() {
            let env_field_types: Vec<BasicTypeEnum<'ctx>> =
                free_vars.values().map(|&(_, ty)| ty).collect();
            let env_struct_type = self.context.struct_type(&env_field_types, false);
            let env_struct_ptr = self.builder.build_pointer_cast(
                env_ptr_param,
                env_struct_type.ptr_type(inkwell::AddressSpace::default()),
                "spawn_env",
            ).map_err(|e| CompileError::LlvmError(format!("pointer cast error: {}", e)))?;
            for (i, (name, &(_, ty))) in free_vars.iter().enumerate() {
                let field_gep = self.builder.build_struct_gep(
                    env_struct_type, env_struct_ptr, i as u32, &format!("spawn_env_{}_gep", name),
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let field_val = self.builder.build_load(ty, field_gep, &format!("spawn_cap_{}", name))
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                let alloca = self.builder.build_alloca(ty, &format!("spawn_cap_{}_alloca", name))
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(alloca, field_val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                wrapper_vars.insert(name.clone(), (alloca, ty));
            }
        }
        
        // Compile the spawn expression using wrapper's own vars (not parent's dangling pointers)
        let result = self.compile_expr(expr, &wrapper_vars)?;
        
        // Allocate heap space for the return value using malloc
        let i64_ty = self.context.i64_type();
        let malloc_fn = self.module.get_function("malloc")
            .ok_or_else(|| "malloc not declared".to_string())?;
        let result_llvm_ty_for_size = result.get_type();
        let byte_size_val = result_llvm_ty_for_size.size_of()
            .and_then(|v: inkwell::values::IntValue<'ctx>| v.get_zero_extended_constant())
            .unwrap_or(0) as u64;
        let byte_size = i64_ty.const_int(byte_size_val, false);
        let result_storage = self.builder.build_call(malloc_fn, &[
            BasicMetadataValueEnum::IntValue(byte_size),
        ], "malloc_result")
            .map_err(|e| CompileError::LlvmError(format!("malloc error: {}", e)))?
            .try_as_basic_value_opt()
            .ok_or("malloc returned void")?;
        let result_storage_ptr = if let BasicValueEnum::PointerValue(pv) = result_storage {
            pv
        } else {
            return Err("malloc should return a pointer".into());
        };
        // Store the result
        let result_llvm_ty = result.get_type();
        let result_ptr_ty = match result_llvm_ty {
            BasicTypeEnum::IntType(t) => t.ptr_type(inkwell::AddressSpace::default()),
            BasicTypeEnum::FloatType(t) => t.ptr_type(inkwell::AddressSpace::default()),
            BasicTypeEnum::PointerType(t) => t.ptr_type(inkwell::AddressSpace::default()),
            BasicTypeEnum::StructType(t) => t.ptr_type(inkwell::AddressSpace::default()),
            BasicTypeEnum::ArrayType(t) => t.ptr_type(inkwell::AddressSpace::default()),
            BasicTypeEnum::VectorType(t) => t.ptr_type(inkwell::AddressSpace::default()),
            BasicTypeEnum::ScalableVectorType(t) => t.ptr_type(inkwell::AddressSpace::default()),
        };
        let result_typed_ptr = self.builder.build_pointer_cast(
            result_storage_ptr,
            result_ptr_ty,
            "result_typed"
        ).map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?;
        self.builder.build_store(result_typed_ptr, result)
            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        // Return the i8* pointer
        self.builder.build_return(Some(&result_storage))
            .map_err(|e| CompileError::LlvmError(format!("return error: {}", e)))?;
        
        // Restore builder position to original block (back in parent function)
        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }
        
        // In the parent function: create heap env struct with captured values
        let capture_arg = if !free_vars.is_empty() {
            let env_field_types: Vec<BasicTypeEnum<'ctx>> =
                free_vars.values().map(|&(_, ty)| ty).collect();
            let env_struct_type = self.context.struct_type(&env_field_types, false);
            let env_byte_size = env_struct_type.size_of()
                .ok_or_else(|| "size_of error".to_string())?;
            let env_heap_ptr = self.builder.build_call(malloc_fn, &[
                BasicMetadataValueEnum::IntValue(env_byte_size),
            ], "spawn_env_heap")
                .map_err(|e| CompileError::LlvmError(format!("malloc error: {}", e)))?
                .try_as_basic_value_opt()
                .ok_or("malloc returned void")?
                .into_pointer_value();
            for (i, (name, &(var_alloca, ty))) in free_vars.iter().enumerate() {
                let val = self.builder.build_load(ty, var_alloca, &format!("spawn_cap_val_{}", name))
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                let field_gep = self.builder.build_struct_gep(
                    env_struct_type, env_heap_ptr, i as u32, &format!("spawn_env_{}_gep", name),
                ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(field_gep, val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
            }
            self.builder.build_pointer_cast(
                env_heap_ptr, i8_ptr, "spawn_env_i8",
            ).map_err(|e| CompileError::LlvmError(format!("pointer cast error: {}", e)))?
        } else {
            i8_ptr.const_null()
        };
        
        let wrapper_fn_ptr = self.builder.build_pointer_cast(
            wrapper_fn.as_global_value().as_pointer_value(),
            i8_ptr,
            "wrapper_i8"
        ).map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?;

        if self.in_parasteps {
            // Parasteps: submit to thread pool
            self.pending_spawn_type = Some(result.get_type());
            let mimi_pool_submit_fn = self.module.get_function("mimi_pool_submit")
                .ok_or("mimi_pool_submit not declared")?;
            self.builder.build_call(mimi_pool_submit_fn, &[
                BasicMetadataValueEnum::PointerValue(wrapper_fn_ptr),
                BasicMetadataValueEnum::PointerValue(capture_arg),
            ], "pool_submit_call")
                .map_err(|e| CompileError::LlvmError(format!("pool_submit error: {}", e)))?;
            let placeholder = i64_ty.const_int(0, false);
            Ok(BasicValueEnum::IntValue(placeholder))
        } else {
            // Non-parasteps: use raw pthread_create
            let thread_alloca = self.builder.build_alloca(i64_ty, "thread")
                .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
            self.builder.build_store(thread_alloca, i64_ty.const_int(0, false))
                .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;

            let pthread_create_fn = self.module.get_function("pthread_create")
                .ok_or("pthread_create not declared")?;
            self.builder.build_call(pthread_create_fn, &[
                BasicMetadataValueEnum::PointerValue(thread_alloca),
                BasicMetadataValueEnum::PointerValue(i8_ptr.const_null()),
                BasicMetadataValueEnum::PointerValue(wrapper_fn_ptr),
                BasicMetadataValueEnum::PointerValue(capture_arg),
            ], "pthread_create_call")
                .map_err(|e| CompileError::LlvmError(format!("pthread_create error: {}", e)))?;

            let thread_id_val = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), thread_alloca, "thread_id")
                .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
            Ok(thread_id_val)
        }
    }

    pub(in crate::codegen) fn compile_await_expr(
        &mut self,
        expr: &Expr,
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        // Await: join the thread and get the result
        let thread_val = self.compile_expr(expr, vars)?;
        let thread_id = match thread_val {
            BasicValueEnum::IntValue(iv) => iv,
            BasicValueEnum::PointerValue(pv) => {
                self.builder.build_load(BasicTypeEnum::IntType(self.context.i64_type()), pv, "thread")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_int_value()
            }
            _ => return Err("await requires a thread (i64) value".into()),
        };
        
        // Allocate space to receive the wrapper's return pointer (void**)
        let i8_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
        let retval_storage = self.builder.build_alloca(i8_ptr, "retval_ptr")
            .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
        self.builder.build_store(retval_storage, i8_ptr.const_null())
            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        
        // Remove from parasteps tracking (already awaited, avoid double-join at block end)
        self.parasteps_thread_ids.retain(|&id| id != thread_id);
        
        let pthread_join_fn = self.module.get_function("pthread_join")
            .ok_or("pthread_join not declared")?;
        self.builder.build_call(pthread_join_fn, &[
            BasicMetadataValueEnum::IntValue(thread_id),
            BasicMetadataValueEnum::PointerValue(retval_storage),
        ], "pthread_join_call")
            .map_err(|e| CompileError::LlvmError(format!("pthread_join error: {}", e)))?;
        
        // Load the returned pointer from the storage (it's the wrapper's malloc'd result)
        let result_i8_ptr = self.builder.build_load(
            BasicTypeEnum::PointerType(i8_ptr),
            retval_storage,
            "result_ptr"
        ).map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
        let result_ptr = if let BasicValueEnum::PointerValue(pv) = result_i8_ptr {
            pv
        } else {
            return Err("expected pointer from pthread_join".into());
        };
        
        // Cast from i8* to result type pointer and load the result value
        let result_type = self.pending_spawn_type.take().unwrap_or_else(|| self.context.i64_type().into());
        let result_typed = self.builder.build_pointer_cast(
            result_ptr,
            result_type.ptr_type(inkwell::AddressSpace::default()),
            "result_typed_ptr"
        ).map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?;
        let result_val = self.builder.build_load(
            result_type,
            result_typed,
            "spawn_result_val"
        ).map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
        
        // Free the malloc'd memory
        let free_fn = self.module.get_function("free")
            .ok_or_else(|| "free not declared".to_string())?;
        self.builder.build_call(free_fn, &[
            BasicMetadataValueEnum::PointerValue(result_ptr),
        ], "free_call")
            .map_err(|e| CompileError::LlvmError(format!("free error: {}", e)))?;
        
        Ok(result_val)
    }

    /// Infer the type name of an object expression from the codegen's type definitions
    /// Build a List<string> from a slice of string values (compile-time constant list)
    pub(in crate::codegen) fn build_string_list(
        &self,
        strings: &[String],
        _vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        let i8_ty = self.context.i8_type();
        let i8_ptr = i8_ty.ptr_type(inkwell::AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let count = strings.len() as u64;

        // Allocate array of string structs: [ { i8*, i64 } x N ]
        let str_ty = self.context.struct_type(&[
            BasicTypeEnum::PointerType(i8_ptr),
            BasicTypeEnum::IntType(i64_ty),
        ], false);
        let arr_type = str_ty.array_type(count as u32);
        let arr_alloca = self.builder.build_alloca(BasicTypeEnum::ArrayType(arr_type), "str_arr")
            .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;

        for (i, s) in strings.iter().enumerate() {
            let global = self.builder.build_global_string_ptr(s, &format!("str_{}", i))
                .map_err(|e| CompileError::LlvmError(format!("global string error: {}", e)))?;
            let elem_ptr = self.builder.build_struct_gep(
                BasicTypeEnum::StructType(str_ty),
                arr_alloca,
                i as u32,
                &format!("elem_{}", i),
            ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
            let ptr_gep = self.builder.build_struct_gep(str_ty, elem_ptr, 0, "ptr")
                .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
            self.builder.build_store(ptr_gep, global.as_pointer_value())
                .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
            let len_gep = self.builder.build_struct_gep(str_ty, elem_ptr, 1, "len")
                .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
            self.builder.build_store(len_gep, i64_ty.const_int(s.len() as u64, false))
                .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        }

        // Build list struct: { i64 len, i8* data }
        let list_ty = self.context.struct_type(&[
            BasicTypeEnum::IntType(i64_ty),
            BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
        ], false);
        let list_alloca = self.builder.build_alloca(list_ty, "str_list")
            .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
        let len_gep = self.builder.build_struct_gep(list_ty, list_alloca, 0, "len")
            .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
        self.builder.build_store(len_gep, i64_ty.const_int(count, false))
            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        let data_gep = self.builder.build_struct_gep(list_ty, list_alloca, 1, "data")
            .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
        let arr_void_ptr = self.builder.build_pointer_cast(
            arr_alloca,
            i8_ptr,
            "arr_void"
        ).map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?;
        self.builder.build_store(data_gep, arr_void_ptr)
            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
        Ok(list_alloca.into())
    }


    /// Determine if an expression evaluates to a string type (for len() dispatch).
    pub(in crate::codegen) fn expr_is_string(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Literal(Lit::String(_)) | Expr::Literal(Lit::FString(_)) => true,
            Expr::Ident(name) => {
                self.var_type_names.get(name).map(|t| t == "string").unwrap_or(false)
            }
            Expr::Call(callee, _) => {
                if let Expr::Ident(name) = callee.as_ref() {
                    matches!(name.as_str(),
                        "to_string" | "int_to_string" | "float_to_string"
                        | "input" | "read_file"
                        | "str_char_at" | "str_substring" | "str_trim"
                        | "str_to_upper" | "str_to_lower" | "str_repeat"
                        | "str_replace" | "str_join"
                        | "type_name" | "from_json" | "c_str_to_string"
                    )
                } else {
                    false
                }
            }
            Expr::Field(_, method) => {
                matches!(method.as_str(),
                    "to_string" | "trim" | "to_upper" | "to_lower"
                    | "repeat" | "replace" | "char_at" | "substring"
                )
            }
            Expr::Turbofish(name, _, _) => {
                matches!(name.as_str(), "to_string")
            }
            _ => false,
        }
    }


    pub(in crate::codegen) fn compile_call(
        &mut self,
        name: &str,
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        let mut compiled_args = Vec::new();
        for arg in args {
            compiled_args.push(self.compile_expr(arg, vars)?);
        }

        // G1b: Convert closure struct args to thunk pointers for extern callback params
        if let Some(param_types) = self.extern_param_types.get(name).cloned() {
            for (i, compiled) in compiled_args.iter_mut().enumerate() {
                if i >= param_types.len() { break; }
                let (cb_params, cb_ret) = match &param_types[i] {
                    crate::ast::Type::ExternFunc(p, r) => (p.as_slice(), r.as_ref()),
                    crate::ast::Type::Func(p, r) => (p.as_slice(), r.as_ref()),
                    _ => continue,
                };
                if let BasicValueEnum::StructValue(sv) = compiled {
                        let struct_ty = sv.get_type();
                        if struct_ty.get_field_types().len() == 2 {
                            let fn_ptr = self.builder.build_extract_value(*sv, 0, "cb_fn_ptr")
                                .map_err(|e| CompileError::LlvmError(format!("extract fn_ptr: {}", e)))?;
                            let env_ptr = self.builder.build_extract_value(*sv, 1, "cb_env_ptr")
                                .map_err(|e| CompileError::LlvmError(format!("extract env_ptr: {}", e)))?;
                            let cb_fn_ptr = fn_ptr.into_pointer_value();
                            let cb_env_ptr = env_ptr.into_pointer_value();
                            let thunk_entry = self.get_or_create_callback_thunk(cb_params, cb_ret)
                                .map_err(|e| CompileError::LlvmError(format!("callback thunk: {}", e)))?;
                            self.builder.build_store(
                                thunk_entry.fn_ptr_global.as_pointer_value(), cb_fn_ptr,
                            ).map_err(|e| CompileError::LlvmError(format!("store fn_ptr: {}", e)))?;
                            self.builder.build_store(
                                thunk_entry.env_ptr_global.as_pointer_value(), cb_env_ptr,
                            ).map_err(|e| CompileError::LlvmError(format!("store env_ptr: {}", e)))?;
                            let i8_ptr_ty = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                            let thunk_ptr = thunk_entry.thunk_fn.as_global_value().as_pointer_value();
                            let casted = self.builder.build_pointer_cast(thunk_ptr, i8_ptr_ty, "thunk_i8")
                                .map_err(|e| CompileError::LlvmError(format!("bitcast thunk: {}", e)))?;
                            *compiled = casted.into();
                        }
                    }
                }
            }

        let metadata_args: Vec<_> = compiled_args.iter().map(|v| {
            match v {
                BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                            BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
            }
        }).collect();

        // Dispatch builtins
        if name == "len" && args.len() == 1 {
            self.pending_len_is_string = self.expr_is_string(&args[0]);
        }
        if crate::codegen::builtins::is_builtin(name) {
            return self.compile_builtin_call(name, &metadata_args).map_err(|e| CompileError::Generic(e.to_string()));
        }

        // Handle built-in Option/Result constructors
        match name {
            "Ok" | "Some" | "Err" | "None" => return self.compile_constructor(name, compiled_args),
            _ => {}
        }

        if let Some(function) = self.module.get_function(name) {
            let call = self.builder.build_call(function, &metadata_args, "call")
                .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?;
            Ok(call_try_basic_value(&call).unwrap_or(
                self.context.i64_type().const_int(0, false).into()
            ))
        } else {
            // Not found by direct name — must be a generic function.
            // Build a callee-specific type_map by inferring generic bindings
            // from the argument types at the call site, instead of using the
            // caller's type_map (which has different generic param names).
            let mangled = if let Some(fdef) = self.func_defs.get(name) {
                if !fdef.generics.is_empty() {
                    let mut callee_map: HashMap<String, Type> = HashMap::new();
                    for gp in &fdef.generics {
                        // Find the first callee param whose type references this generic
                        for (i, param) in fdef.params.iter().enumerate() {
                            if i < args.len() && Self::type_references_generic(&param.ty, &gp.name) {
                                if let Some(arg_type) = self.expr_type_of(&args[i], vars) {
                                    callee_map.insert(gp.name.clone(), arg_type);
                                    break;
                                }
                            }
                        }
                    }
                    Self::mangle_name(name, &callee_map)
                } else {
                    Self::mangle_name(name, &self.type_map)
                }
            } else {
                Self::mangle_name(name, &self.type_map)
            };

            if let Some(function) = self.module.get_function(&mangled) {
                let call = self.builder.build_call(function, &metadata_args, "call")
                    .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?;
                Ok(call_try_basic_value(&call).unwrap_or(
                    self.context.i64_type().const_int(0, false).into()
                ))
            } else {
                Err(format!("undefined function '{}' in codegen", name).into())
            }
        }
    }


    pub(in crate::codegen) fn compile_constructor(
        &mut self,
        name: &str,
        compiled_args: Vec<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        match name {
            "Ok" => {
                if compiled_args.len() != 1 {
                    return Err("Ok expects 1 argument".into());
                }
                let val = compiled_args[0];
                let bool_ty = self.context.bool_type();
                let i64_ty = self.context.i64_type();
                let disc = bool_ty.const_int(1, false);
                let inner_ty = val.get_type();
                let struct_ty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(bool_ty),
                    inner_ty,
                    BasicTypeEnum::IntType(i64_ty),
                ], false);
                let alloca = self.builder.build_alloca(struct_ty, "ok_val")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                let disc_gep = self.builder.build_struct_gep(struct_ty, alloca, 0, "disc")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(disc_gep, disc)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let val_gep = self.builder.build_struct_gep(struct_ty, alloca, 1, "payload")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(val_gep, val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let err_gep = self.builder.build_struct_gep(struct_ty, alloca, 2, "err_pad")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(err_gep, i64_ty.const_int(0, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let result = self.builder.build_load(struct_ty, alloca, "loaded")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                Ok(result)
            }
            "Some" => {
                if compiled_args.len() != 1 {
                    return Err("Some expects 1 argument".into());
                }
                let val = compiled_args[0];
                let bool_ty = self.context.bool_type();
                let disc = bool_ty.const_int(1, false);
                let inner_ty = val.get_type();
                let struct_ty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(bool_ty),
                    inner_ty,
                ], false);
                let alloca = self.builder.build_alloca(struct_ty, "some_val")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                let disc_gep = self.builder.build_struct_gep(struct_ty, alloca, 0, "disc")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(disc_gep, disc)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let val_gep = self.builder.build_struct_gep(struct_ty, alloca, 1, "payload")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(val_gep, val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let result = self.builder.build_load(struct_ty, alloca, "loaded")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                Ok(result)
            }
            "Err" => {
                if compiled_args.len() != 1 {
                    return Err("Err expects 1 argument".into());
                }
                let val = compiled_args[0];
                let bool_ty = self.context.bool_type();
                let i64_ty = self.context.i64_type();
                let disc = bool_ty.const_int(0, false);
                let err_val: BasicValueEnum = match val {
                    BasicValueEnum::IntValue(iv) => {
                        let bit_width = iv.get_type().get_bit_width();
                        if bit_width < 64 {
                            self.builder.build_int_s_extend(iv, i64_ty, "err_sext")
                                .map_err(|e| CompileError::LlvmError(format!("int sign extend error: {}", e)))?
                                .into()
                        } else if bit_width > 64 {
                            self.builder.build_int_truncate(iv, i64_ty, "err_trunc")
                                .map_err(|e| CompileError::LlvmError(format!("int truncate error: {}", e)))?
                                .into()
                        } else {
                            iv.into()
                        }
                    }
                    BasicValueEnum::PointerValue(pv) => {
                        self.builder.build_ptr_to_int(pv, i64_ty, "err_to_i64")
                            .map_err(|e| CompileError::LlvmError(format!("ptrtoint error: {}", e)))?
                            .into()
                    }
                    _ => return Err("Err: unsupported error value type".into()),
                };
                let struct_ty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(bool_ty),
                    BasicTypeEnum::IntType(i64_ty),
                    BasicTypeEnum::IntType(i64_ty),
                ], false);
                let alloca = self.builder.build_alloca(struct_ty, "err_val")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                let disc_gep = self.builder.build_struct_gep(struct_ty, alloca, 0, "disc")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(disc_gep, disc)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let ok_gep = self.builder.build_struct_gep(struct_ty, alloca, 1, "ok_pad")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(ok_gep, i64_ty.const_int(0, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let err_gep = self.builder.build_struct_gep(struct_ty, alloca, 2, "err_payload")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(err_gep, err_val)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let result = self.builder.build_load(struct_ty, alloca, "loaded")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                Ok(result)
            }
            "None" => {
                if compiled_args.len() != 0 {
                    return Err("None expects 0 arguments".into());
                }
                let bool_ty = self.context.bool_type();
                let i64_ty = self.context.i64_type();
                let disc = bool_ty.const_int(0, false);
                let struct_ty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(bool_ty),
                    BasicTypeEnum::IntType(i64_ty),
                ], false);
                let alloca = self.builder.build_alloca(struct_ty, "none_val")
                    .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                let disc_gep = self.builder.build_struct_gep(struct_ty, alloca, 0, "disc")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(disc_gep, disc)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let val_gep = self.builder.build_struct_gep(struct_ty, alloca, 1, "payload")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                self.builder.build_store(val_gep, i64_ty.const_int(0, false))
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                let result = self.builder.build_load(struct_ty, alloca, "loaded")
                    .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                Ok(result)
            }
            _ => Err(format!("unknown constructor '{}'", name).into()),
        }
    }


    /// Determine the Mimi Type of an expression by resolving through the
    /// caller's type_map. Used to infer callee generic bindings at call sites.
    pub(in crate::codegen) fn expr_type_of(&self, expr: &Expr, _vars: &HashMap<String, VarEntry<'ctx>>) -> Option<Type> {
        match expr {
            Expr::Ident(name) => {
                if let Some(tn) = self.var_type_names.get(name) {
                    let raw = Type::Name(tn.clone(), vec![]);
                    Some(self.resolve_type(&raw))
                } else {
                    None
                }
            }
            _ => None,
        }
    }


    /// Check whether a Type contains a reference to a generic parameter name.
    pub(in crate::codegen) fn type_references_generic(ty: &Type, generic_name: &str) -> bool {
        match ty {
            Type::Name(name, args) => {
                if name == generic_name {
                    return true;
                }
                args.iter().any(|a| Self::type_references_generic(a, generic_name))
            }
            Type::Ref(_, inner) | Type::RefMut(_, inner) => Self::type_references_generic(inner, generic_name),
            Type::Option(inner) => Self::type_references_generic(inner, generic_name),
            Type::Result(ok, err) => {
                Self::type_references_generic(ok, generic_name)
                    || Self::type_references_generic(err, generic_name)
            }
            Type::Tuple(elems) => elems.iter().any(|e| Self::type_references_generic(e, generic_name)),
            Type::Func(args, ret) => {
                args.iter().any(|a| Self::type_references_generic(a, generic_name))
                    || Self::type_references_generic(ret, generic_name)
            }
            Type::Shared(inner) | Type::LocalShared(inner) | Type::Weak(inner) | Type::WeakLocal(inner)
            | Type::RawPtr(inner) | Type::RawPtrMut(inner) | Type::CShared(inner)
            | Type::CBorrow(inner) | Type::CBorrowMut(inner) | Type::Slice(inner)
            | Type::CBuffer(inner) | Type::Array(inner, _) => {
                Self::type_references_generic(inner, generic_name)
            }
            Type::Newtype(_, inner) => Self::type_references_generic(inner, generic_name),
            Type::ExternFunc(args, ret) => {
                args.iter().any(|a| Self::type_references_generic(a, generic_name))
                    || Self::type_references_generic(ret, generic_name)
            }
            Type::Cap(_) | Type::Nothing | Type::Allocator | Type::Infer
            | Type::ImplTrait(_) | Type::DynTrait(_) | Type::RawString => false,
        }
    }


    /// Call a function by its mangled name
    pub(in crate::codegen) fn compile_call_mangled(
        &mut self,
        mangled: &str,
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        let mut compiled_args = Vec::new();
        for arg in args {
            compiled_args.push(self.compile_expr(arg, vars)?);
        }

        let metadata_args: Vec<_> = compiled_args.iter().map(|v| {
            match v {
                BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                            BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(self.context.i64_type().const_int(0, false)),
            }
        }).collect();

        if let Some(function) = self.module.get_function(mangled) {
            let call = self.builder.build_call(function, &metadata_args, "call")
                .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?;
            Ok(call_try_basic_value(&call).unwrap_or(
                self.context.i64_type().const_int(0, false).into()
            ))
        } else {
            Err(format!("undefined function '{}' in codegen", mangled).into())
        }
    }


    /// Find a FuncDef by name from the codegen's stored func_defs
    pub(in crate::codegen) fn find_func_def(&self, name: &str) -> Result<FuncDef, CompileError> {
        self.func_defs.get(name)
            .cloned()
            .ok_or_else(|| CompileError::Generic(format!("function '{}' definition not available for monomorphization", name)))
    }

}
