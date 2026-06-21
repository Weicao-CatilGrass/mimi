use crate::ast::*;
use crate::codegen::{CodeGenerator, VarEntry};
use crate::error::CompileError;

use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;
use std::collections::HashMap;

impl<'ctx> CodeGenerator<'ctx> {
    pub(in crate::codegen) fn bind_pattern_variables(
        &mut self,
        arm: &MatchArm,
        scrutinee_val: BasicValueEnum<'ctx>,
        scrutinee_iv: inkwell::values::IntValue<'ctx>,
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<HashMap<String, VarEntry<'ctx>>, CompileError> {
        let mut local_vars = vars.clone();
        // Bind variables from pattern
        match &arm.pat {
            Pattern::Variable(name) => {
                let alloca = self.builder.build_alloca(
                    BasicTypeEnum::IntType(self.context.i64_type()),
                    name,
                ).map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                self.builder.build_store(alloca, scrutinee_iv)
                    .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                local_vars.insert(name.clone(), (alloca, BasicTypeEnum::IntType(self.context.i64_type())));
            }
            Pattern::Constructor(_, inner_patterns) => {
                // For constructor patterns, bind inner variables
                // For now, assume single inner variable
                for inner_pat in inner_patterns {
                    if let Pattern::Variable(name) = inner_pat {
                        let alloca = self.builder.build_alloca(
                            BasicTypeEnum::IntType(self.context.i64_type()),
                            name,
                        ).map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                        self.builder.build_store(alloca, scrutinee_iv)
                            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                        local_vars.insert(name.clone(), (alloca, BasicTypeEnum::IntType(self.context.i64_type())));
                    }
                }
            }
            Pattern::Tuple(inner_pats) => {
                // For tuple patterns, bind inner variables by loading from struct
                let scrutinee_ptr = match scrutinee_val {
                    BasicValueEnum::PointerValue(pv) => pv,
                    _ => return Ok(local_vars),
                };
                // Determine tuple element types from the struct
                let _elem_count = inner_pats.len();
                for (j, inner_pat) in inner_pats.iter().enumerate() {
                    if let Pattern::Variable(name) = inner_pat {
                        let gep = self.builder.build_struct_gep(
                            BasicTypeEnum::IntType(self.context.i64_type()),
                            scrutinee_ptr,
                            j as u32,
                            &format!("tuple_{}", j),
                        ).map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                        let val = self.builder.build_load(
                            BasicTypeEnum::IntType(self.context.i64_type()),
                            gep,
                            &format!("tup_{}", j),
                        ).map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                        let alloca = self.builder.build_alloca(
                            BasicTypeEnum::IntType(self.context.i64_type()),
                            name,
                        ).map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                        self.builder.build_store(alloca, val)
                            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                        local_vars.insert(name.clone(), (alloca, BasicTypeEnum::IntType(self.context.i64_type())));
                    }
                }
            }
            Pattern::Array(inner_pats) => {
                // For array patterns, bind inner variables by loading from list data
                let scrutinee_ptr = match scrutinee_val {
                    BasicValueEnum::PointerValue(pv) => pv,
                    _ => return Ok(local_vars),
                };
                // Load data pointer from list struct
                let list_ty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(self.context.i64_type()),
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                ], false);
                let data_gep = self.builder.build_struct_gep(list_ty, scrutinee_ptr, 1, "list_data")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let data_i8 = self.builder.build_load(
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                    data_gep, "data").map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_pointer_value();
                let i64_ty = self.context.i64_type();
                let data_ptr = self.builder.build_bit_cast(data_i8,
                    i64_ty.ptr_type(inkwell::AddressSpace::default()), "data_i64")
                    .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?.into_pointer_value();
                for (j, inner_pat) in inner_pats.iter().enumerate() {
                    if let Pattern::Variable(name) = inner_pat {
                        let idx = i64_ty.const_int(j as u64, false);
                        // SAFETY: build_gep requires valid pointer and index types; the pointer is derived from a valid LLVM-typed allocation and indices are correctly-typed i64 values.
                        let elem_ptr = unsafe {
                            self.builder.build_gep(i64_ty, data_ptr, &[idx], &format!("arr_{}", j))
                        }.map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                        let val = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), elem_ptr, &format!("arrv_{}", j))
                            .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                        let alloca = self.builder.build_alloca(BasicTypeEnum::IntType(i64_ty), name)
                            .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                        self.builder.build_store(alloca, val)
                            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                        local_vars.insert(name.clone(), (alloca, BasicTypeEnum::IntType(i64_ty)));
                    }
                }
            }
            Pattern::Slice(inner_pats, rest) => {
                // For slice patterns, bind prefix variables and rest as list
                let scrutinee_ptr = match scrutinee_val {
                    BasicValueEnum::PointerValue(pv) => pv,
                    _ => return Ok(local_vars),
                };
                let list_ty = self.context.struct_type(&[
                    BasicTypeEnum::IntType(self.context.i64_type()),
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                ], false);
                let data_gep = self.builder.build_struct_gep(list_ty, scrutinee_ptr, 1, "list_data")
                    .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                let data_i8 = self.builder.build_load(
                    BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                    data_gep, "data").map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?.into_pointer_value();
                let i64_ty = self.context.i64_type();
                let data_ptr = self.builder.build_bit_cast(data_i8,
                    i64_ty.ptr_type(inkwell::AddressSpace::default()), "data_i64")
                    .map_err(|e| CompileError::LlvmError(format!("bitcast error: {}", e)))?.into_pointer_value();
                // Bind prefix elements
                for (j, inner_pat) in inner_pats.iter().enumerate() {
                    if let Pattern::Variable(name) = inner_pat {
                        let idx = i64_ty.const_int(j as u64, false);
                        // SAFETY: build_gep requires valid pointer and index types; the pointer is derived from a valid LLVM-typed allocation and indices are correctly-typed i64 values.
                        let elem_ptr = unsafe {
                            self.builder.build_gep(i64_ty, data_ptr, &[idx], &format!("slc_{}", j))
                        }.map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                        let val = self.builder.build_load(BasicTypeEnum::IntType(i64_ty), elem_ptr, &format!("slcv_{}", j))
                            .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                        let alloca = self.builder.build_alloca(BasicTypeEnum::IntType(i64_ty), name)
                            .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                        self.builder.build_store(alloca, val)
                            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                        local_vars.insert(name.clone(), (alloca, BasicTypeEnum::IntType(i64_ty)));
                    }
                }
                // Bind rest as remaining list (simplified: bind as empty list)
                if let Some(rest_pat) = rest.as_ref() {
                    if let Pattern::Variable(name) = rest_pat.as_ref() {
                        let i64_ty = self.context.i64_type();
                        let empty_list: BasicValueEnum = i64_ty.const_int(0, false).into();
                        let alloca = self.builder.build_alloca(BasicTypeEnum::IntType(i64_ty), name)
                            .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                        self.builder.build_store(alloca, empty_list)
                            .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                        local_vars.insert(name.clone(), (alloca, BasicTypeEnum::IntType(i64_ty)));
                    }
                }
            }
            Pattern::Wildcard | Pattern::Literal(_) => {
                // Wildcard and literal patterns: no variable binding needed
            }
        }
        Ok(local_vars)
    }


    /// Generate element-wise comparison for a tuple pattern.
    /// Returns `Some(i1)` if any element requires comparison, `None` for wildcard-only patterns.
    fn compile_tuple_pattern(
        &self,
        scrutinee: BasicValueEnum<'ctx>,
        inner_pats: &[Pattern],
    ) -> Result<Option<inkwell::values::IntValue<'ctx>>, CompileError> {
        let i64_ty = self.context.i64_type();
        let tuple_ptr = match scrutinee {
            BasicValueEnum::PointerValue(pv) => pv,
            BasicValueEnum::StructValue(sv) => {
                let alloca = self.builder.build_alloca(sv.get_type(), "tuple_alloca")
                    .map_err(|e| CompileError::LlvmError(format!("alloca: {}", e)))?;
                self.builder.build_store(alloca, sv)
                    .map_err(|e| CompileError::LlvmError(format!("store: {}", e)))?;
                alloca
            }
            _ => return Err("tuple pattern requires struct value".into()),
        };
        let mut agg: Option<inkwell::values::IntValue<'ctx>> = None;
        for (j, pat) in inner_pats.iter().enumerate() {
            let lit_val = match pat {
                Pattern::Literal(lit) => match lit {
                    Lit::Int(n) => Some(i64_ty.const_int(*n as u64, true)),
                    Lit::Bool(b) => Some(i64_ty.const_int(*b as u64, false)),
                    Lit::Unit => Some(i64_ty.const_int(0, false)),
                    _ => return Err("unsupported tuple element literal type".into()),
                },
                _ => None,
            };
            if let Some(expected) = lit_val {
                let elem_ptr = self.builder.build_struct_gep(
                    BasicTypeEnum::IntType(i64_ty), tuple_ptr, j as u32,
                    &format!("tup_el{}", j),
                ).map_err(|e| CompileError::LlvmError(format!("gep: {}", e)))?;
                let elem_val = self.builder.build_load(
                    BasicTypeEnum::IntType(i64_ty), elem_ptr, &format!("tup_v{}", j),
                ).map_err(|e| CompileError::LlvmError(format!("load: {}", e)))?
                    .into_int_value();
                let eq = self.builder.build_int_compare(
                    inkwell::IntPredicate::EQ, elem_val, expected, &format!("tup_cmp{}", j),
                ).map_err(|e| CompileError::LlvmError(format!("cmp: {}", e)))?;
                agg = Some(match agg {
                    Some(prev) => self.builder.build_and(prev, eq, "tup_and")
                        .map_err(|e| CompileError::LlvmError(format!("and: {}", e)))?,
                    None => eq,
                });
            }
        }
        Ok(agg)
    }

    /// Generate element-wise comparison for an array pattern.
    /// Returns `Some(i1)` if any element requires comparison, `None` for wildcard-only patterns.
    fn compile_array_pattern(
        &self,
        scrutinee: BasicValueEnum<'ctx>,
        inner_pats: &[Pattern],
    ) -> Result<Option<inkwell::values::IntValue<'ctx>>, CompileError> {
        let i64_ty = self.context.i64_type();
        let scrutinee_ptr = match scrutinee {
            BasicValueEnum::PointerValue(pv) => pv,
            _ => return Ok(None),
        };
        let list_ty = self.context.struct_type(&[
            BasicTypeEnum::IntType(i64_ty),
            BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
        ], false);
        let data_gep = self.builder.build_struct_gep(list_ty, scrutinee_ptr, 1, "list_data")
            .map_err(|e| CompileError::LlvmError(format!("gep: {}", e)))?;
        let data_i8 = self.builder.build_load(
            BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
            data_gep, "data",
        ).map_err(|e| CompileError::LlvmError(format!("load: {}", e)))?.into_pointer_value();
        let data_ptr = self.builder.build_bit_cast(data_i8,
            i64_ty.ptr_type(inkwell::AddressSpace::default()), "data_i64")
            .map_err(|e| CompileError::LlvmError(format!("bitcast: {}", e)))?.into_pointer_value();
        let mut agg: Option<inkwell::values::IntValue<'ctx>> = None;
        for (j, pat) in inner_pats.iter().enumerate() {
            let lit_val = match pat {
                Pattern::Literal(lit) => match lit {
                    Lit::Int(n) => Some(i64_ty.const_int(*n as u64, true)),
                    Lit::Bool(b) => Some(i64_ty.const_int(*b as u64, false)),
                    Lit::Unit => Some(i64_ty.const_int(0, false)),
                    _ => return Err("unsupported array element literal type".into()),
                },
                _ => None,
            };
            if let Some(expected) = lit_val {
                let idx = i64_ty.const_int(j as u64, false);
                // SAFETY: pointer derived from valid list data allocation
                let elem_ptr = unsafe {
                    self.builder.build_gep(i64_ty, data_ptr, &[idx], &format!("arr_el{}", j))
                }.map_err(|e| CompileError::LlvmError(format!("gep: {}", e)))?;
                let elem_val = self.builder.build_load(
                    BasicTypeEnum::IntType(i64_ty), elem_ptr, &format!("arr_v{}", j),
                ).map_err(|e| CompileError::LlvmError(format!("load: {}", e)))?
                    .into_int_value();
                let eq = self.builder.build_int_compare(
                    inkwell::IntPredicate::EQ, elem_val, expected, &format!("arr_cmp{}", j),
                ).map_err(|e| CompileError::LlvmError(format!("cmp: {}", e)))?;
                agg = Some(match agg {
                    Some(prev) => self.builder.build_and(prev, eq, "arr_and")
                        .map_err(|e| CompileError::LlvmError(format!("and: {}", e)))?,
                    None => eq,
                });
            }
        }
        Ok(agg)
    }

    /// Generate element-wise comparison for a slice pattern.
    /// Returns `Some(i1)` if any element requires comparison, `None` for wildcard-only patterns.
    fn compile_slice_pattern(
        &self,
        scrutinee: BasicValueEnum<'ctx>,
        inner_pats: &[Pattern],
        _rest: &Option<Box<Pattern>>,
    ) -> Result<Option<inkwell::values::IntValue<'ctx>>, CompileError> {
        // Same data access as array patterns, only comparing prefix elements
        self.compile_array_pattern(scrutinee, inner_pats)
    }

    pub(in crate::codegen) fn compile_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CompileError> {
        let scrutinee_val = self.compile_expr(scrutinee, vars)?;
        let scrutinee_iv = match scrutinee_val {
            BasicValueEnum::IntValue(iv) => iv,
            BasicValueEnum::StructValue(sv) => {
                let tag = self.builder.build_extract_value(sv, 0, "enum_tag")
                    .map_err(|e| CompileError::LlvmError(format!("extract enum tag: {}", e)))?
                    .into_int_value();
                self.builder.build_int_cast(tag, self.context.i64_type(), "tag_ext")
                    .map_err(|e| CompileError::LlvmError(format!("extend tag: {}", e)))?
            }
            BasicValueEnum::PointerValue(pv) => {
                let i32_ty = self.context.i32_type();
                let struct_ptr = self.builder.build_struct_gep(
                    BasicTypeEnum::IntType(i32_ty), pv, 0, "tag_gep",
                ).map_err(|e| CompileError::LlvmError(format!("tag gep: {}", e)))?;
                let tag = self.builder.build_load(
                    BasicTypeEnum::IntType(i32_ty), struct_ptr, "tag_load",
                ).map_err(|e| CompileError::LlvmError(format!("tag load: {}", e)))?
                    .into_int_value();
                self.builder.build_int_cast(tag, self.context.i64_type(), "tag_ext")
                    .map_err(|e| CompileError::LlvmError(format!("extend tag: {}", e)))?
            }
            _ => return Err("match scrutinee must be integer or enum struct".into()),
        };

        let function = self.current_function().ok_or_else(|| "codegen: no current function for match".to_string())?;
        let merge_bb = self.context.append_basic_block(function, "matchcont");
        let mut else_bb = self.context.append_basic_block(function, "matchelse");

        // Branch from current block to the dispatch (matchelse)
        self.builder.build_unconditional_branch(else_bb)
            .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
        self.builder.position_at_end(else_bb);

        let mut incoming_vals = Vec::new();
        let mut incoming_bbs = Vec::new();

        // Build if-else chain for each arm
        for (i, arm) in arms.iter().enumerate() {
            let arm_bb = self.context.append_basic_block(function, &format!("arm{}", i));

            match &arm.pat {
                Pattern::Wildcard | Pattern::Variable(_) => {
                    // Always matches - jump to arm body
                    self.builder.position_at_end(else_bb);
                    self.builder.build_unconditional_branch(arm_bb)
                        .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                    // Create a fresh else_bb so the after-loop code doesn't
                    // double-terminate the block we just wrote to.
                    else_bb = self.context.append_basic_block(function, &format!("wccont{}", i));
                }
                Pattern::Literal(lit) => {
                    self.builder.position_at_end(else_bb);
                    let lit_val = match lit {
                        Lit::Int(n) => self.context.i64_type().const_int(*n as u64, true),
                        Lit::Bool(b) => self.context.bool_type().const_int(*b as u64, false),
                        Lit::Unit => self.context.i64_type().const_int(0, false),
                        _ => return Err("unsupported match literal type".into()),
                    };
                    let cmp = self.builder.build_int_compare(
                        inkwell::IntPredicate::EQ,
                        scrutinee_iv,
                        lit_val,
                        "cmp",
                    ).map_err(|e| CompileError::LlvmError(format!("cmp error: {}", e)))?;
                    // Always create an intermediate next block so the else chain
                    // never points directly at merge_bb.  This keeps the phi's
                    // predecessor set clean and avoids corrupting merge_bb.
                    let next_bb = self.context.append_basic_block(function, &format!("next{}", i));
                    self.builder.build_conditional_branch(cmp, arm_bb, next_bb)
                        .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                    else_bb = next_bb;
                }
                Pattern::Constructor(name, _) => {
                    // Constructor pattern: compare tag using ordinal index
                    self.builder.position_at_end(else_bb);
                    // Look up the variant ordinal index from type definitions
                    let ordinal = self.find_variant_ordinal(name)
                        .map_err(|e| CompileError::LlvmError(format!("match arm variant lookup: {}", e)))?;
                    let tag_val = self.context.i64_type().const_int(ordinal, false);
                    let cmp = self.builder.build_int_compare(
                        inkwell::IntPredicate::EQ,
                        scrutinee_iv,
                        tag_val,
                        "cmp",
                    ).map_err(|e| CompileError::LlvmError(format!("cmp error: {}", e)))?;
                    let next_bb = self.context.append_basic_block(function, &format!("next{}", i));
                    self.builder.build_conditional_branch(cmp, arm_bb, next_bb)
                        .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                    else_bb = next_bb;
                }
                Pattern::Tuple(inner_pats) => {
                    self.builder.position_at_end(else_bb);
                    let match_cmp = self.compile_tuple_pattern(scrutinee_val, inner_pats)?;
                    let next_bb = self.context.append_basic_block(function, &format!("next{}", i));
                    match match_cmp {
                        Some(cmp) => {
                            self.builder.build_conditional_branch(cmp, arm_bb, next_bb)
                                .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                        }
                        None => {
                            self.builder.build_unconditional_branch(arm_bb)
                                .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                        }
                    }
                    else_bb = next_bb;
                }
                Pattern::Array(inner_pats) => {
                    self.builder.position_at_end(else_bb);
                    let match_cmp = self.compile_array_pattern(scrutinee_val, inner_pats)?;
                    let next_bb = self.context.append_basic_block(function, &format!("next{}", i));
                    match match_cmp {
                        Some(cmp) => {
                            self.builder.build_conditional_branch(cmp, arm_bb, next_bb)
                                .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                        }
                        None => {
                            self.builder.build_unconditional_branch(arm_bb)
                                .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                        }
                    }
                    else_bb = next_bb;
                }
                Pattern::Slice(inner_pats, rest) => {
                    self.builder.position_at_end(else_bb);
                    let match_cmp = self.compile_slice_pattern(scrutinee_val, inner_pats, rest)?;
                    let next_bb = self.context.append_basic_block(function, &format!("next{}", i));
                    match match_cmp {
                        Some(cmp) => {
                            self.builder.build_conditional_branch(cmp, arm_bb, next_bb)
                                .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                        }
                        None => {
                            self.builder.build_unconditional_branch(arm_bb)
                                .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                        }
                    }
                    else_bb = next_bb;
                }
            }

            // Arm body — bind pattern variables
            self.builder.position_at_end(arm_bb);
            let local_vars = self.bind_pattern_variables(arm, scrutinee_val, scrutinee_iv, vars)?;
            match &arm.guard {
                Some(guard) => {
                    let guard_val = self.compile_expr(guard, &local_vars)?;
                    let guard_bool = match guard_val {
                        BasicValueEnum::IntValue(iv) => {
                            self.builder.build_int_compare(
                                inkwell::IntPredicate::NE,
                                iv,
                                self.context.i64_type().const_int(0, false),
                                "guard_cmp",
                            ).map_err(|e| CompileError::LlvmError(format!("guard cmp: {}", e)))?
                        }
                        BasicValueEnum::PointerValue(pv) => {
                            // Not-null means truthy (non-null pointers are valid values)
                            let is_null = self.builder.build_is_null(pv, "guard_null")
                                .map_err(|e| CompileError::LlvmError(format!("guard null: {}", e)))?;
                            let zero = self.context.bool_type().const_int(0, false);
                            self.builder.build_int_compare(
                                inkwell::IntPredicate::EQ, is_null, zero, "guard_notnull",
                            ).map_err(|e| CompileError::LlvmError(format!("guard notnull: {}", e)))?
                        }
                        _ => return Err("match guard must be boolean or pointer".into()),
                    };
                    let arm_body_bb = self.context.append_basic_block(function, &format!("arm_body{}", i));
                    self.builder.build_conditional_branch(guard_bool, arm_body_bb, else_bb)
                        .map_err(|e| CompileError::LlvmError(format!("guard branch: {}", e)))?;
                    self.builder.position_at_end(arm_body_bb);
                    let arm_val = self.compile_expr(&arm.body, &local_vars)?;
                    incoming_vals.push(arm_val);
                    incoming_bbs.push(arm_body_bb);
                    self.builder.build_unconditional_branch(merge_bb)
                        .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                }
                None => {
                    let arm_val = self.compile_expr(&arm.body, &local_vars)?;
                    incoming_vals.push(arm_val);
                    incoming_bbs.push(arm_bb);
                    self.builder.build_unconditional_branch(merge_bb)
                        .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;
                }
            }
        }

        // Unreachable else block (should not be reached if match is exhaustive).
        // else_bb is a fresh next_N block (never merge_bb) thanks to the
        // unconditional intermediate-block creation above.
        self.builder.position_at_end(else_bb);
        self.builder.build_unconditional_branch(merge_bb)
            .map_err(|e| CompileError::LlvmError(format!("branch error: {}", e)))?;

        // Merge block - use phi to select the right value
        self.builder.position_at_end(merge_bb);
        if incoming_vals.is_empty() {
            return Err("empty match expression".into());
        }
        let ty = incoming_vals[0].get_type();
        let phi = self.builder.build_phi(ty, "match.result")
            .map_err(|e| CompileError::LlvmError(format!("phi error: {}", e)))?;
        let mut phi_incoming: Vec<_> = incoming_vals.iter().zip(incoming_bbs.iter())
            .map(|(v, bb)| (v as &dyn inkwell::values::BasicValue, *bb))
            .collect();
        // Add the unreachable else block with a dummy value so every
        // predecessor of merge_bb has a phi entry.
        let dummy_val = self.context.i64_type().const_int(0, false);
        phi_incoming.push((&dummy_val as &dyn inkwell::values::BasicValue, else_bb));
        phi.add_incoming(&phi_incoming);
        Ok(phi.as_basic_value())
    }

}
