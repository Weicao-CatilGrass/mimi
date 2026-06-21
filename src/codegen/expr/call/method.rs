use crate::ast::*;
use crate::codegen::{call_try_basic_value, CodeGenerator, VarEntry};
use crate::error::CompileError;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};
use std::collections::HashMap;

impl<'ctx> CodeGenerator<'ctx> {
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
}
