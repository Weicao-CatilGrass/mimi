use crate::ast::*;
use crate::codegen::{call_try_basic_value, CodeGenerator, VarEntry};
use crate::error::CompileError;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};
use std::collections::HashMap;

impl<'ctx> CodeGenerator<'ctx> {
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
                // Check if this is a first-class function pointer variable
                if self.fn_ptr_var_names.contains(name.as_str()) {
                    if let Some(&(alloca, ty)) = vars.get(name.as_str()) {
                        let fn_ptr = self.builder.build_load(ty, alloca, &format!("{}_fn", name))
                            .map_err(|e| CompileError::LlvmError(format!("fn ptr load error: {}", e)))?
                            .into_pointer_value();
                        let mut compiled_args = Vec::new();
                        for arg in args {
                            compiled_args.push(self.compile_expr(arg, vars)?);
                        }
                        let i64_ty = self.context.i64_type();
                        let mut all_meta = Vec::new();
                        for arg in &compiled_args {
                            all_meta.push(match arg {
                                BasicValueEnum::IntValue(iv) => BasicMetadataTypeEnum::IntType(iv.get_type()),
                                BasicValueEnum::FloatValue(fv) => BasicMetadataTypeEnum::FloatType(fv.get_type()),
                                BasicValueEnum::PointerValue(pv) => BasicMetadataTypeEnum::PointerType(pv.get_type()),
                                BasicValueEnum::StructValue(sv) => BasicMetadataTypeEnum::StructType(sv.get_type()),
                                BasicValueEnum::ArrayValue(av) => BasicMetadataTypeEnum::ArrayType(av.get_type()),
                                BasicValueEnum::VectorValue(vv) => BasicMetadataTypeEnum::VectorType(vv.get_type()),
                                BasicValueEnum::ScalableVectorValue(_) => BasicMetadataTypeEnum::IntType(i64_ty),
                            });
                        }
                        let ret_type = i64_ty;
                        let indirect_fn_type = ret_type.fn_type(&all_meta, false);
                        let fn_ptr_typed = self.builder.build_pointer_cast(
                            fn_ptr,
                            self.context.ptr_type(inkwell::AddressSpace::default()),
                            "fn_typed",
                        ).map_err(|e| CompileError::LlvmError(format!("pointer cast error: {}", e)))?;
                        let call_args: Vec<_> = compiled_args.iter().map(|arg| {
                            match arg {
                                BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(*iv),
                                BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(*fv),
                                BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(*pv),
                                BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(*sv),
                                BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(*av),
                                BasicValueEnum::VectorValue(vv) => BasicMetadataValueEnum::VectorValue(*vv),
                                BasicValueEnum::ScalableVectorValue(_) => BasicMetadataValueEnum::IntValue(i64_ty.const_int(0, false)),
                            }
                        }).collect();
                        let call = self.builder.build_indirect_call(
                            indirect_fn_type, fn_ptr_typed, &call_args, "fn_ptr_call",
                        ).map_err(|e| CompileError::LlvmError(format!("fn ptr call error: {}", e)))?;
                        return Ok(call_try_basic_value(&call).unwrap_or(
                            i64_ty.const_int(0, false).into()
                        ));
                    }
                }
                // Check if this is a closure variable call
                if let Some(&(alloca, BasicTypeEnum::StructType(st))) = vars.get(name.as_str()) {
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
                            let i8_ptr = self.context.ptr_type(inkwell::AddressSpace::default());
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
                                self.context.ptr_type(inkwell::AddressSpace::default()),
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
                self.compile_call(name, args, vars)
            }
            Expr::Field(obj, method_name) => {
                // Namespaced enum constructor: TypeName::Variant(args)
                if let Expr::Ident(type_name) = obj.as_ref() {
                    let is_builtin_enum = type_name == "Result" || type_name == "Option";
                    let is_custom_enum = self.type_defs.get(type_name)
                        .map(|td| matches!(td.kind, crate::ast::TypeDefKind::Enum(_)))
                        .unwrap_or(false);
                    if is_builtin_enum {
                        // Result::Ok/Err or Option::Some/None
                        return self.compile_call(method_name, args, vars);
                    }
                    if is_custom_enum {
                        let ctor_name = format!("{}_{}", type_name, method_name);
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
                    }
                }
                self.compile_method_call(obj, method_name, args, vars)
            }
            _ => Err("only direct function calls and method calls supported in codegen".into()),
        }
    }
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
                let i8_ptr = self.context.ptr_type(inkwell::AddressSpace::default());
                let fn_type = i64_ty.fn_type(&[
                    BasicMetadataTypeEnum::PointerType(i8_ptr),
                    BasicMetadataTypeEnum::IntType(i64_ty),
                ], false);
                let fn_typed = self.builder.build_pointer_cast(
                    fn_ptr, self.context.ptr_type(inkwell::AddressSpace::default()), "fn_typed"
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
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                ], false);
                let loaded = self.builder.build_load(BasicTypeEnum::StructType(closure_struct_ty), pv, "closure_loaded")
                    .map_err(|e| CompileError::LlvmError(format!("load closure error: {}", e)))?.into_struct_value();
                let fn_ptr = self.builder.build_extract_value(loaded, 0, "fn_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract fn_ptr error: {}", e)))?.into_pointer_value();
                let env_ptr = self.builder.build_extract_value(loaded, 1, "env_ptr")
                    .map_err(|e| CompileError::LlvmError(format!("extract env_ptr error: {}", e)))?.into_pointer_value();
                let i8_ptr = self.context.ptr_type(inkwell::AddressSpace::default());
                let fn_type = i64_ty.fn_type(&[
                    BasicMetadataTypeEnum::PointerType(i8_ptr),
                    BasicMetadataTypeEnum::IntType(i64_ty),
                ], false);
                let fn_typed = self.builder.build_pointer_cast(
                    fn_ptr, self.context.ptr_type(inkwell::AddressSpace::default()), "fn_typed"
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
                            self.pending_callback_tls.push(thunk_entry.fn_ptr_global.as_pointer_value());
                            self.pending_callback_tls.push(thunk_entry.env_ptr_global.as_pointer_value());
                            let i8_ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                            let thunk_ptr = thunk_entry.thunk_fn.as_global_value().as_pointer_value();
                            let casted = self.builder.build_pointer_cast(thunk_ptr, i8_ptr_ty, "thunk_i8")
                                .map_err(|e| CompileError::LlvmError(format!("bitcast thunk: {}", e)))?;
                            *compiled = casted.into();
                        }
                    }
                }
            }

        // For extern functions: load struct values from pointers for repr(C) struct-by-value params.
        // compile_record_expr stores the struct on the stack and returns a PointerValue, but extern
        // wrappers expect struct values passed by value per the C ABI. Without this load, the pointer
        // address bits get interpreted as struct fields, producing garbage (F-16: LLVM ABI mismatch).
        if self.extern_func_defs.contains_key(name) {
            if let Some(ef) = self.extern_func_defs.get(name) {
                for (i, arg) in compiled_args.iter_mut().enumerate() {
                    if i >= ef.params.len() { break; }
                    if let crate::ast::Type::Name(n, _) = &ef.params[i].ty {
                        if self.repr_c_record_names.contains(n.as_str()) {
                            if let BasicValueEnum::PointerValue(pv) = arg {
                                if let Some(&BasicTypeEnum::StructType(sty)) = self.type_llvm.get(n.as_str()) {
                                    let loaded = self.builder.build_load(
                                        BasicTypeEnum::StructType(sty), *pv,
                                        &format!("{}_extern_val", n),
                                    ).map_err(|e| CompileError::LlvmError(format!("load struct for extern: {}", e)))?;
                                    *arg = loaded;
                                }
                            }
                        }
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

        // Route enum variant constructors to their registered TypeName_VariantName
        // functions. These functions are emitted in register_type_def for every
        // enum variant. This takes precedence over the built-in Option/Result
        // constructors so that user-defined enums with variants named Some/None/Ok/Err
        // use the custom layout.
        if let Some((type_name, _ordinal)) = self.find_variant_owner(name) {
            let ctor_name = format!("{}_{}", type_name, name);
            if let Some(function) = self.module.get_function(&ctor_name) {
                let call = self.builder.build_call(function, &metadata_args, "call")
                    .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?;
                return Ok(call_try_basic_value(&call).unwrap_or(
                    self.context.i64_type().const_int(0, false).into()
                ));
            }
            return Err(format!("enum constructor '{}' not registered", ctor_name).into());
        }

        // Built-in Option/Result constructors (only used when no custom enum owns the name).
        match name {
            "Ok" | "Some" | "Err" | "None" => return self.compile_constructor(name, compiled_args),
            _ => {}
        }

        // ── Argument ABI conversions for regular (non-builtin) function calls ──

        // Convert pointer-valued list arguments to struct values when the
        // function parameter expects List<T> (passed by value). Only applies
        // to List types, not other struct types (e.g., func(T)->U closure).
        if let Some(fdef) = self.func_defs.get(name) {
            for (i, arg) in compiled_args.iter_mut().enumerate() {
                if i < fdef.params.len() {
                    if let Type::Name(tn, _) = &fdef.params[i].ty {
                        if tn == "List" {
                            if let Some(param_llvm) = self.llvm_type_for(&fdef.params[i].ty) {
                                if let BasicValueEnum::PointerValue(pv) = arg {
                                    let loaded = self.builder.build_load(
                                        param_llvm, *pv,
                                        &format!("{}_struct_arg", &fdef.params[i].name),
                                    ).map_err(|e| CompileError::LlvmError(format!("load struct arg: {}", e)))?;
                                    *arg = loaded;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Convert function pointers to closure structs when the parameter type
        // expects func(T) -> U. Named functions are compiled as i8* pointers,
        // but func(T) -> U parameters expect {i8*, i8*} closure structs.
        // For named functions, generate a thunk wrapper that accepts the closure
        // ABI (env_ptr as first param) and forwards to the original function.
        let fn_names_for_wrapping: Vec<Option<String>> = self.func_defs.get(name).map(|fdef| {
            args.iter().enumerate().map(|(i, arg_expr)| {
                if i < fdef.params.len() && matches!(&fdef.params[i].ty, Type::Func(_, _)) {
                    if let Expr::Ident(fn_name) = arg_expr {
                        return Some(fn_name.clone());
                    }
                }
                None
            }).collect()
        }).unwrap_or_default();
        // Create wrappers outside the func_defs borrow
        let mut wrapper_cache: Vec<Option<inkwell::values::PointerValue<'ctx>>> = Vec::new();
        for fn_name_opt in &fn_names_for_wrapping {
            if let Some(fn_name) = fn_name_opt {
                wrapper_cache.push(Some(self.get_or_create_closure_wrapper(fn_name)?));
            } else {
                wrapper_cache.push(None);
            }
        }
        // Apply wrappers to compiled_args
        for (i, arg) in compiled_args.iter_mut().enumerate() {
            if let Some(Some(wrapper)) = wrapper_cache.get(i) {
                if let BasicValueEnum::PointerValue(_pv) = arg {
                    let closure_ty = crate::codegen::types::closure_struct_type(self.context);
                    let closure_alloca = self.builder.build_alloca(
                        BasicTypeEnum::StructType(closure_ty), "closure_arg",
                    ).map_err(|e| CompileError::LlvmError(format!("closure alloca: {}", e)))?;
                    let fn_gep = self.gep().build_struct_gep(closure_ty, closure_alloca, 0, "fn_gep")
                        .map_err(|e| CompileError::LlvmError(format!("fn gep: {}", e)))?;
                    self.builder.build_store(fn_gep, BasicValueEnum::PointerValue(*wrapper))
                        .map_err(|e| CompileError::LlvmError(format!("fn store: {}", e)))?;
                    let env_gep = self.gep().build_struct_gep(closure_ty, closure_alloca, 1, "env_gep")
                        .map_err(|e| CompileError::LlvmError(format!("env gep: {}", e)))?;
                    let null_i8 = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();
                    self.builder.build_store(env_gep, BasicValueEnum::PointerValue(null_i8))
                        .map_err(|e| CompileError::LlvmError(format!("env store: {}", e)))?;
                    let loaded = self.builder.build_load(
                        BasicTypeEnum::StructType(closure_ty), closure_alloca, "closure_loaded",
                    ).map_err(|e| CompileError::LlvmError(format!("load closure: {}", e)))?;
                    *arg = loaded;
                }
            }
        }

        // Rebuild metadata_args after ABI conversions
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

        // Check if this is a lazily-generated extern function
            if self.extern_func_defs.contains_key(name) {
                self.generate_extern_fn(name)?;
            }
            if let Some(function) = self.module.get_function(name) {
            let call = self.builder.build_call(function, &metadata_args, "call")
                .map_err(|e| CompileError::LlvmError(format!("call error: {}", e)))?;
            // Clear callback TLS globals after the call to prevent stale data
            // from being read by re-entrant callbacks or subsequent calls.
            let i8_ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
            let null_i8 = i8_ptr_ty.const_null();
            for tls_ptr in self.pending_callback_tls.drain(..) {
                self.builder.build_store(tls_ptr, null_i8)
                    .map_err(|e| CompileError::LlvmError(format!("clear tls: {}", e)))?;
            }
            // If calling an async function, record the inner result type for await.
            if let Some(fdef) = self.func_defs.get(name) {
                if fdef.is_async {
                    if let Some(ret_ty) = &fdef.ret {
                        if let Some(llvm_ret) = self.llvm_type_for(ret_ty) {
                            self.pending_spawn_type = Some(llvm_ret);
                        }
                    }
                }
            }
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
                let msg = if self.comptime_func_names.contains(name) {
                    format!("comptime function '{}' is compile-time only and cannot be called from runtime code", name)
                } else {
                    format!("undefined function '{}' in codegen", name)
                };
                Err(msg.into())
            }
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
            let msg = if self.comptime_func_names.contains(mangled) {
                format!("comptime function '{}' is compile-time only and cannot be called from runtime code", mangled)
            } else {
                format!("undefined function '{}' in codegen", mangled)
            };
            Err(msg.into())
        }
    }
    /// Get or create a closure ABI wrapper for a named function.
    /// Named functions compile with direct ABI `fn(params...) -> ret`, but when
    /// passed as a `func(T) -> U` parameter, the caller expects the closure ABI
    /// `fn(env_ptr: i8*, params...) -> ret`. This method generates a tiny wrapper
    /// that ignores env_ptr and forwards all params to the original function.
    pub(in crate::codegen) fn get_or_create_closure_wrapper(
        &mut self,
        name: &str,
    ) -> Result<inkwell::values::PointerValue<'ctx>, CompileError> {
        // Check cache first
        if let Some(cached) = self.closure_wrappers.get(name) {
            return Ok(*cached);
        }

        let orig_fn = self.module.get_function(name)
            .ok_or_else(|| CompileError::Generic(format!("cannot create closure wrapper for unknown function '{}'", name)))?;
        let fn_type = orig_fn.get_type();
        let param_tys = fn_type.get_param_types();
        let ret_ty = fn_type.get_return_type()
            .ok_or_else(|| CompileError::Generic(format!("closure wrapper: function '{}' has void return type", name)))?;

        let i8_ptr = self.context.ptr_type(inkwell::AddressSpace::default());

        // Build wrapper function type: fn(i8*, params...) -> ret
        let mut wrapper_params: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::new();
        wrapper_params.push(BasicMetadataTypeEnum::PointerType(i8_ptr)); // env_ptr
        for pt in &param_tys {
            wrapper_params.push(*pt);
        }

        let wrapper_fn_type = match ret_ty {
            BasicTypeEnum::IntType(t) => t.fn_type(&wrapper_params, false),
            BasicTypeEnum::FloatType(t) => t.fn_type(&wrapper_params, false),
            BasicTypeEnum::PointerType(t) => t.fn_type(&wrapper_params, false),
            BasicTypeEnum::StructType(t) => t.fn_type(&wrapper_params, false),
            BasicTypeEnum::ArrayType(t) => t.fn_type(&wrapper_params, false),
            _ => return Err(CompileError::Generic(format!("closure wrapper: unsupported return type for '{}'", name))),
        };

        let wrapper_name = format!("__mimi_fn_wrapper_{}", name.replace('.', "_"));
        let wrapper_fn = self.module.add_function(&wrapper_name, wrapper_fn_type, Some(inkwell::module::Linkage::Internal));

        let saved_block = self.builder.get_insert_block();
        let entry_bb = self.context.append_basic_block(wrapper_fn, "entry");
        self.builder.position_at_end(entry_bb);

        // Forward all params (skip env_ptr at index 0) to the original function
        let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
        for i in 0..param_tys.len() {
            let param = wrapper_fn.get_nth_param((i + 1) as u32) // +1 for env_ptr
                .ok_or_else(|| CompileError::LlvmError(format!("wrapper: param {} not found", i + 1)))?;
            call_args.push(match param {
                BasicValueEnum::IntValue(iv) => BasicMetadataValueEnum::IntValue(iv),
                BasicValueEnum::FloatValue(fv) => BasicMetadataValueEnum::FloatValue(fv),
                BasicValueEnum::PointerValue(pv) => BasicMetadataValueEnum::PointerValue(pv),
                BasicValueEnum::StructValue(sv) => BasicMetadataValueEnum::StructValue(sv),
                BasicValueEnum::ArrayValue(av) => BasicMetadataValueEnum::ArrayValue(av),
                _ => return Err(CompileError::LlvmError(format!("wrapper: unsupported param type at {}", i + 1))),
            });
        }

        let call = self.builder.build_call(orig_fn, &call_args, "wrapper_call")
            .map_err(|e| CompileError::LlvmError(format!("wrapper call: {}", e)))?;
        let ret_val = crate::codegen::call_try_basic_value(&call)
            .ok_or_else(|| CompileError::LlvmError("wrapper call returned void".to_string()))?;
        self.builder.build_return(Some(&ret_val))
            .map_err(|e| CompileError::LlvmError(format!("wrapper return: {}", e)))?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        let wrapper_ptr = wrapper_fn.as_global_value().as_pointer_value();
        self.closure_wrappers.insert(name.to_string(), wrapper_ptr);
        Ok(wrapper_ptr)
    }

    /// Find a FuncDef by name from the codegen's stored func_defs
    pub(in crate::codegen) fn find_func_def(&self, name: &str) -> Result<FuncDef, CompileError> {
        self.func_defs.get(name)
            .cloned()
            .ok_or_else(|| CompileError::Generic(format!("function '{}' definition not available for monomorphization", name)))
    }
}
