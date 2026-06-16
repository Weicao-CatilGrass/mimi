#![allow(dead_code, deprecated)]

pub mod types;

use crate::ast::*;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;
use inkwell::OptimizationLevel;
use std::collections::HashMap;
use std::path::Path;

pub struct CodeGenerator<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    loop_break: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    loop_continue: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    type_defs: HashMap<String, crate::ast::TypeDef>,
    type_llvm: HashMap<String, BasicTypeEnum<'ctx>>,
}

type VarEntry<'ctx> = (inkwell::values::PointerValue<'ctx>, BasicTypeEnum<'ctx>);

impl<'ctx> CodeGenerator<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self { context, module, builder, loop_break: None, loop_continue: None, type_defs: HashMap::new(), type_llvm: HashMap::new() }
    }

    pub fn compile_file(&mut self, file: &File) -> Result<(), String> {
        // First pass: collect type definitions
        for item in &file.items {
            match item {
                Item::Type(t) => {
                    self.register_type_def(t)?;
                }
                Item::Module(m) => {
                    for inner in &m.items {
                        if let Item::Type(t) = inner {
                            self.register_type_def(t)?;
                        }
                    }
                }
                _ => {}
            }
        }
        // Second pass: compile functions
        for item in &file.items {
            match item {
                Item::Func(f) if !f.is_comptime => self.compile_func(f)?,
                Item::Module(m) => {
                    for inner in &m.items {
                        if let Item::Func(f) = inner {
                            if !f.is_comptime {
                                self.compile_func(f)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn register_type_def(&mut self, t: &crate::ast::TypeDef) -> Result<(), String> {
        let llvm_ty = match &t.kind {
            crate::ast::TypeDefKind::Record(fields) => {
                let mut field_tys = Vec::new();
                for f in fields {
                    let ty = types::mimi_type_to_llvm(self.context, &f.ty)
                        .unwrap_or(BasicTypeEnum::IntType(self.context.i64_type()));
                    field_tys.push(ty);
                }
                BasicTypeEnum::StructType(self.context.struct_type(&field_tys, false))
            }
            crate::ast::TypeDefKind::Enum(_variants) => {
                // Enum representation: i32 tag + union of largest variant payload
                let tag_ty = BasicTypeEnum::IntType(self.context.i32_type());
                let payload_ty = BasicTypeEnum::IntType(self.context.i64_type());
                // For simplicity, use i64 as payload storage
                BasicTypeEnum::StructType(self.context.struct_type(&[tag_ty, payload_ty], false))
            }
            crate::ast::TypeDefKind::Alias(ty) | crate::ast::TypeDefKind::Newtype(ty) => {
                types::mimi_type_to_llvm(self.context, ty)
                    .unwrap_or(BasicTypeEnum::IntType(self.context.i64_type()))
            }
        };
        self.type_llvm.insert(t.name.clone(), llvm_ty);
        self.type_defs.insert(t.name.clone(), t.clone());
        Ok(())
    }

    fn compile_func(&mut self, func: &FuncDef) -> Result<(), String> {
        let ret_type = match &func.ret {
            Some(ty) => types::mimi_type_to_llvm(self.context, ty)
                .unwrap_or(BasicTypeEnum::IntType(self.context.i64_type())),
            None => BasicTypeEnum::IntType(self.context.i64_type()),
        };

        let mut param_types = Vec::new();
        for param in &func.params {
            if let Some(ty) = types::mimi_type_to_llvm(self.context, &param.ty) {
                param_types.push(ty);
            }
        }

        let metadata_params: Vec<_> = param_types.iter().map(|t| types::basic_to_metadata(self.context, *t)).collect();

        let fn_type = match ret_type {
            BasicTypeEnum::IntType(t) => t.fn_type(&metadata_params, false),
            BasicTypeEnum::FloatType(t) => t.fn_type(&metadata_params, false),
            BasicTypeEnum::PointerType(t) => t.fn_type(&metadata_params, false),
            BasicTypeEnum::StructType(t) => t.fn_type(&metadata_params, false),
            BasicTypeEnum::ArrayType(t) => t.fn_type(&metadata_params, false),
            _ => self.context.i64_type().fn_type(&metadata_params, false),
        };

        let function = self.module.add_function(&func.name, fn_type, None);
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        let mut vars: HashMap<String, VarEntry<'ctx>> = HashMap::new();
        for (i, param) in func.params.iter().enumerate() {
            if let Some(ty) = types::mimi_type_to_llvm(self.context, &param.ty) {
                let alloca = self.builder.build_alloca(ty, &param.name)
                    .map_err(|e| format!("alloca error: {}", e))?;
                self.builder.build_store(alloca, function.get_nth_param(i as u32).expect("param index matches function signature"))
                    .map_err(|e| format!("store error: {}", e))?;
                vars.insert(param.name.clone(), (alloca, ty));
            }
        }

        let mut last_val: BasicValueEnum = self.context.i64_type().const_int(0, false).into();
        for stmt in &func.body {
            match stmt {
                Stmt::Expr(expr) => {
                    last_val = self.compile_expr(expr, &vars)?;
                }
                Stmt::Return(Some(expr)) => {
                    let val = self.compile_expr(expr, &vars)?;
                    self.builder.build_return(Some(&val)).map_err(|e| format!("return error: {}", e))?;
                    return Ok(());
                }
                Stmt::Return(None) => {
                    self.builder.build_return(None).map_err(|e| format!("return error: {}", e))?;
                    return Ok(());
                }
                Stmt::Let { pat, init: Some(init), .. } => {
                    let val = self.compile_expr(init, &vars)?;
                    let name = match pat {
                        Pattern::Variable(n) => n.clone(),
                        _ => continue,
                    };
                    let ty = val.get_type();
                    let alloca = self.builder.build_alloca(ty, &name)
                        .map_err(|e| format!("alloca error: {}", e))?;
                    self.builder.build_store(alloca, val)
                        .map_err(|e| format!("store error: {}", e))?;
                    vars.insert(name, (alloca, ty));
                }
                Stmt::Assign { target: Expr::Ident(name), value } => {
                    let val = self.compile_expr(value, &vars)?;
                    if let Some(&(alloca, _)) = vars.get(name) {
                        self.builder.build_store(alloca, val)
                            .map_err(|e| format!("store error: {}", e))?;
                    }
                }
                Stmt::If { cond, then_, else_ } => {
                    let cond_val = self.compile_expr(cond, &vars)?;
                    let cond_bool = if let BasicValueEnum::IntValue(iv) = cond_val {
                        iv
                    } else {
                        return Err("if condition must be boolean".into());
                    };

                    let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                    let then_bb = self.context.append_basic_block(function, "then");
                    let else_bb = self.context.append_basic_block(function, "else");
                    let merge_bb = self.context.append_basic_block(function, "ifcont");

                    self.builder.build_conditional_branch(cond_bool, then_bb, else_bb)
                        .map_err(|e| format!("branch error: {}", e))?;

                    // Then block
                    self.builder.position_at_end(then_bb);
                    self.compile_block(then_, &mut vars)?;
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb)
                            .map_err(|e| format!("branch error: {}", e))?;
                    }

                    // Else block
                    self.builder.position_at_end(else_bb);
                    if let Some(else_block) = else_ {
                        self.compile_block(else_block, &mut vars)?;
                    }
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb)
                            .map_err(|e| format!("branch error: {}", e))?;
                    }

                    // Continue at merge
                    self.builder.position_at_end(merge_bb);
                }
                Stmt::While { cond, body } => {
                    let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                    let loop_bb = self.context.append_basic_block(function, "loop");
                    let body_bb = self.context.append_basic_block(function, "loopbody");
                    let merge_bb = self.context.append_basic_block(function, "loopcont");

                    // Jump to loop condition check
                    self.builder.build_unconditional_branch(loop_bb)
                        .map_err(|e| format!("branch error: {}", e))?;

                    // Loop condition
                    self.builder.position_at_end(loop_bb);
                    let cond_val = self.compile_expr(cond, &vars)?;
                    let cond_bool = if let BasicValueEnum::IntValue(iv) = cond_val {
                        iv
                    } else {
                        return Err("while condition must be boolean".into());
                    };
                    self.builder.build_conditional_branch(cond_bool, body_bb, merge_bb)
                        .map_err(|e| format!("branch error: {}", e))?;

                    // Loop body
                    self.builder.position_at_end(body_bb);
                    let old_break = self.loop_break.take();
                    let old_continue = self.loop_continue.take();
                    self.loop_break = Some(merge_bb);
                    self.loop_continue = Some(loop_bb);
                    self.compile_block(body, &mut vars)?;
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(loop_bb)
                            .map_err(|e| format!("branch error: {}", e))?;
                    }
                    self.loop_break = old_break;
                    self.loop_continue = old_continue;

                    // Continue after loop
                    self.builder.position_at_end(merge_bb);
                }
                Stmt::For { var, iterable, body } => {
                    // For loop: compile to while loop over index
                    let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                    let iterable_val = self.compile_expr(iterable, &vars)?;
                    let list_ptr = match iterable_val {
                        BasicValueEnum::PointerValue(pv) => pv,
                        _ => return Err("for loop requires a list".into()),
                    };

                    // Get list length (first element of list struct)
                    let list_struct_ty = inkwell::types::BasicTypeEnum::StructType(
                        self.context.struct_type(&[
                            BasicTypeEnum::IntType(self.context.i64_type()),
                            BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                        ], false)
                    );
                    let list_len_gep = self.builder.build_struct_gep(
                        list_struct_ty,
                        list_ptr,
                        0,
                        "list.len"
                    ).map_err(|e| format!("gep error: {}", e))?;
                    let list_len = self.builder.build_load(
                        BasicTypeEnum::IntType(self.context.i64_type()),
                        list_len_gep,
                        "len"
                    ).map_err(|e| format!("load error: {}", e))?;

                    // Create index variable
                    let idx_alloca = self.builder.build_alloca(self.context.i64_type(), "idx")
                        .map_err(|e| format!("alloca error: {}", e))?;
                    self.builder.build_store(idx_alloca, self.context.i64_type().const_int(0, false))
                        .map_err(|e| format!("store error: {}", e))?;

                    // Loop condition: idx < len
                    let loop_bb = self.context.append_basic_block(function, "forloop");
                    let body_bb = self.context.append_basic_block(function, "forbody");
                    let merge_bb = self.context.append_basic_block(function, "forcont");

                    self.builder.build_unconditional_branch(loop_bb)
                        .map_err(|e| format!("branch error: {}", e))?;

                    self.builder.position_at_end(loop_bb);
                    let idx_val = self.builder.build_load(
                        BasicTypeEnum::IntType(self.context.i64_type()),
                        idx_alloca,
                        "idx"
                    ).map_err(|e| format!("load error: {}", e))?;
                    let idx_iv = if let BasicValueEnum::IntValue(iv) = idx_val { iv } else { return Err("index must be i64".into()); };
                    let len_iv = if let BasicValueEnum::IntValue(iv) = list_len { iv } else { return Err("length must be i64".into()); };
                    let cmp = self.builder.build_int_compare(inkwell::IntPredicate::SLT, idx_iv, len_iv, "cmp")
                        .map_err(|e| format!("cmp error: {}", e))?;
                    self.builder.build_conditional_branch(cmp, body_bb, merge_bb)
                        .map_err(|e| format!("branch error: {}", e))?;

                    // Body
                    self.builder.position_at_end(body_bb);
                    let old_break = self.loop_break.take();
                    let old_continue = self.loop_continue.take();
                    self.loop_break = Some(merge_bb);
                    self.loop_continue = Some(loop_bb);

                    // Get list data pointer
                    let data_gep = self.builder.build_struct_gep(
                        list_struct_ty,
                        list_ptr,
                        1,
                        "list.data"
                    ).map_err(|e| format!("gep error: {}", e))?;
                    let data_ptr = self.builder.build_load(
                        BasicTypeEnum::PointerType(self.context.ptr_type(inkwell::AddressSpace::default())),
                        data_gep,
                        "data"
                    ).map_err(|e| format!("load error: {}", e))?;
                    let data_pv = if let BasicValueEnum::PointerValue(pv) = data_ptr { pv } else { return Err("data must be pointer".into()); };

                    // GEP to get element
                    let elem_ptr = unsafe {
                        self.builder.build_gep(
                            BasicTypeEnum::IntType(self.context.i64_type()),
                            data_pv,
                            &[idx_iv],
                            "elem"
                        )
                    }.map_err(|e| format!("gep error: {}", e))?;
                    let elem = self.builder.build_load(
                        BasicTypeEnum::IntType(self.context.i64_type()),
                        elem_ptr,
                        "elem_val"
                    ).map_err(|e| format!("load error: {}", e))?;

                    // Bind loop variable
                    let elem_alloca = self.builder.build_alloca(BasicTypeEnum::IntType(self.context.i64_type()), var)
                        .map_err(|e| format!("alloca error: {}", e))?;
                    self.builder.build_store(elem_alloca, elem)
                        .map_err(|e| format!("store error: {}", e))?;
                    vars.insert(var.clone(), (elem_alloca, BasicTypeEnum::IntType(self.context.i64_type())));

                    // Compile body
                    self.compile_block(body, &mut vars)?;

                    // Increment index
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        let next_idx = self.builder.build_int_add(idx_iv, self.context.i64_type().const_int(1, false), "next_idx")
                            .map_err(|e| format!("add error: {}", e))?;
                        self.builder.build_store(idx_alloca, next_idx)
                            .map_err(|e| format!("store error: {}", e))?;
                        self.builder.build_unconditional_branch(loop_bb)
                            .map_err(|e| format!("branch error: {}", e))?;
                    }

                    self.loop_break = old_break;
                    self.loop_continue = old_continue;
                    self.builder.position_at_end(merge_bb);
                }
                Stmt::Break(_) => {
                    if let Some(target) = self.loop_break {
                        self.builder.build_unconditional_branch(target)
                            .map_err(|e| format!("break error: {}", e))?;
                        // Create unreachable block for subsequent statements
                        let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                        let unreachable = self.context.append_basic_block(function, "unreachable");
                        self.builder.position_at_end(unreachable);
                    } else {
                        return Err("break outside of loop".into());
                    }
                }
                Stmt::Continue => {
                    if let Some(target) = self.loop_continue {
                        self.builder.build_unconditional_branch(target)
                            .map_err(|e| format!("continue error: {}", e))?;
                        let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                        let unreachable = self.context.append_basic_block(function, "unreachable");
                        self.builder.position_at_end(unreachable);
                    } else {
                        return Err("continue outside of loop".into());
                    }
                }
                _ => {}
            }
        }

        self.builder.build_return(Some(&last_val)).map_err(|e| format!("return error: {}", e))?;
        Ok(())
    }

    fn compile_block(
        &mut self,
        block: &Block,
        vars: &mut HashMap<String, VarEntry<'ctx>>,
    ) -> Result<(), String> {
        for stmt in block {
            match stmt {
                Stmt::Expr(expr) => {
                    self.compile_expr(expr, vars)?;
                }
                Stmt::Return(Some(expr)) => {
                    let val = self.compile_expr(expr, vars)?;
                    self.builder.build_return(Some(&val)).map_err(|e| format!("return error: {}", e))?;
                    return Ok(());
                }
                Stmt::Return(None) => {
                    self.builder.build_return(None).map_err(|e| format!("return error: {}", e))?;
                    return Ok(());
                }
                Stmt::Let { pat, init: Some(init), .. } => {
                    let val = self.compile_expr(init, vars)?;
                    let name = match pat {
                        Pattern::Variable(n) => n.clone(),
                        _ => continue,
                    };
                    let ty = val.get_type();
                    let alloca = self.builder.build_alloca(ty, &name)
                        .map_err(|e| format!("alloca error: {}", e))?;
                    self.builder.build_store(alloca, val)
                        .map_err(|e| format!("store error: {}", e))?;
                    vars.insert(name, (alloca, ty));
                }
                Stmt::Assign { target: Expr::Ident(name), value } => {
                    let val = self.compile_expr(value, vars)?;
                    if let Some(&(alloca, _)) = vars.get(name) {
                        self.builder.build_store(alloca, val)
                            .map_err(|e| format!("store error: {}", e))?;
                    }
                }
                Stmt::If { cond, then_, else_ } => {
                    let cond_val = self.compile_expr(cond, vars)?;
                    let cond_bool = if let BasicValueEnum::IntValue(iv) = cond_val {
                        iv
                    } else {
                        return Err("if condition must be boolean".into());
                    };

                    let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                    let then_bb = self.context.append_basic_block(function, "then");
                    let else_bb = self.context.append_basic_block(function, "else");
                    let merge_bb = self.context.append_basic_block(function, "ifcont");

                    self.builder.build_conditional_branch(cond_bool, then_bb, else_bb)
                        .map_err(|e| format!("branch error: {}", e))?;

                    self.builder.position_at_end(then_bb);
                    self.compile_block(then_, vars)?;
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb)
                            .map_err(|e| format!("branch error: {}", e))?;
                    }

                    self.builder.position_at_end(else_bb);
                    if let Some(else_block) = else_ {
                        self.compile_block(else_block, vars)?;
                    }
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb)
                            .map_err(|e| format!("branch error: {}", e))?;
                    }

                    self.builder.position_at_end(merge_bb);
                }
                Stmt::Break(_) | Stmt::Continue => {}
                _ => {}
            }
        }
        Ok(())
    }

    fn compile_expr(
        &self,
        expr: &Expr,
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match expr {
            Expr::Literal(lit) => match lit {
                Lit::Int(n) => Ok(self.context.i64_type().const_int(*n as u64, true).into()),
                Lit::Float(f) => Ok(self.context.f64_type().const_float(*f).into()),
                Lit::Bool(b) => Ok(self.context.bool_type().const_int(*b as u64, false).into()),
                Lit::Unit => Ok(self.context.i64_type().const_int(0, false).into()),
                Lit::String(s) => {
                    let global = self.builder.build_global_string_ptr(s, "str")
                        .map_err(|e| format!("string error: {}", e))?;
                    Ok(global.as_pointer_value().into())
                }
                Lit::FString(_) => Ok(self.context.i64_type().const_int(0, false).into()),
            },
            Expr::Ident(name) => {
                if let Some(&(alloca, ty)) = vars.get(name) {
                    self.builder.build_load(ty, alloca, name)
                        .map_err(|e| format!("load error: {}", e))
                } else {
                    Err(format!("undefined variable '{}'", name))
                }
            }
            Expr::Binary(op, lhs, rhs) => {
                let l = self.compile_expr(lhs, vars)?;
                let r = self.compile_expr(rhs, vars)?;
                self.compile_binop(*op, l, r)
            }
            Expr::Unary(op, inner) => {
                let v = self.compile_expr(inner, vars)?;
                match op {
                    UnOp::Neg => {
                        if let BasicValueEnum::IntValue(iv) = v {
                            let zero = self.context.i64_type().const_int(0, true);
                            Ok(self.builder.build_int_sub(zero, iv, "neg")
                                .map_err(|e| format!("neg error: {}", e))?.into())
                        } else if let BasicValueEnum::FloatValue(fv) = v {
                            let zero = self.context.f64_type().const_float(0.0);
                            Ok(self.builder.build_float_sub(zero, fv, "fneg")
                                .map_err(|e| format!("neg error: {}", e))?.into())
                        } else {
                            Err("negation requires numeric type".into())
                        }
                    }
                    UnOp::Not => {
                        if let BasicValueEnum::IntValue(iv) = v {
                            Ok(self.builder.build_not(iv, "not")
                                .map_err(|e| format!("not error: {}", e))?.into())
                        } else {
                            Err("not requires boolean type".into())
                        }
                    }
                    _ => Err(format!("unsupported unary operator {:?}", op)),
                }
            }
            Expr::Call(callee, args) => {
                if let Expr::Ident(name) = callee.as_ref() {
                    self.compile_call(name, args, vars)
                } else {
                    Err("only direct function calls supported in codegen".into())
                }
            }
            Expr::Match(scrutinee, arms) => {
                let scrutinee_val = self.compile_expr(scrutinee, vars)?;
                let scrutinee_iv = if let BasicValueEnum::IntValue(iv) = scrutinee_val {
                    iv
                } else {
                    return Err("match scrutinee must be integer (enum tag)".into());
                };

                let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let merge_bb = self.context.append_basic_block(function, "matchcont");
                let mut else_bb = self.context.append_basic_block(function, "matchelse");

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
                                .map_err(|e| format!("branch error: {}", e))?;
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
                            ).map_err(|e| format!("cmp error: {}", e))?;
                            let next_bb = if i < arms.len() - 1 {
                                self.context.append_basic_block(function, &format!("next{}", i))
                            } else {
                                merge_bb
                            };
                            self.builder.build_conditional_branch(cmp, arm_bb, next_bb)
                                .map_err(|e| format!("branch error: {}", e))?;
                            else_bb = next_bb;
                        }
                        Pattern::Constructor(name, _) => {
                            // Constructor pattern: compare tag (name hash as i64 for now)
                            self.builder.position_at_end(else_bb);
                            let tag_val = self.context.i64_type().const_int(
                                name.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64)),
                                false,
                            );
                            let cmp = self.builder.build_int_compare(
                                inkwell::IntPredicate::EQ,
                                scrutinee_iv,
                                tag_val,
                                "cmp",
                            ).map_err(|e| format!("cmp error: {}", e))?;
                            let next_bb = if i < arms.len() - 1 {
                                self.context.append_basic_block(function, &format!("next{}", i))
                            } else {
                                merge_bb
                            };
                            self.builder.build_conditional_branch(cmp, arm_bb, next_bb)
                                .map_err(|e| format!("branch error: {}", e))?;
                            else_bb = next_bb;
                        }
                        _ => return Err(format!("unsupported pattern in codegen: {:?}", arm.pat)),
                    }

                    // Arm body
                    self.builder.position_at_end(arm_bb);
                    let mut local_vars = vars.clone();
                    // Bind variables from pattern
                    match &arm.pat {
                        Pattern::Variable(name) => {
                            let alloca = self.builder.build_alloca(
                                BasicTypeEnum::IntType(self.context.i64_type()),
                                name,
                            ).map_err(|e| format!("alloca error: {}", e))?;
                            self.builder.build_store(alloca, scrutinee_iv)
                                .map_err(|e| format!("store error: {}", e))?;
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
                                    ).map_err(|e| format!("alloca error: {}", e))?;
                                    self.builder.build_store(alloca, scrutinee_iv)
                                        .map_err(|e| format!("store error: {}", e))?;
                                    local_vars.insert(name.clone(), (alloca, BasicTypeEnum::IntType(self.context.i64_type())));
                                }
                            }
                        }
                        _ => {}
                    }
                    let arm_val = self.compile_expr(&arm.body, &local_vars)?;
                    incoming_vals.push(arm_val);
                    incoming_bbs.push(arm_bb);
                    self.builder.build_unconditional_branch(merge_bb)
                        .map_err(|e| format!("branch error: {}", e))?;
                }

                // Unreachable else block (should not be reached if match is exhaustive)
                self.builder.position_at_end(else_bb);
                self.builder.build_unconditional_branch(merge_bb)
                    .map_err(|e| format!("branch error: {}", e))?;

                // Merge block - use phi to select the right value
                self.builder.position_at_end(merge_bb);
                if incoming_vals.is_empty() {
                    return Err("empty match expression".into());
                }
                let ty = incoming_vals[0].get_type();
                let phi = self.builder.build_phi(ty, "match.result")
                    .map_err(|e| format!("phi error: {}", e))?;
                let phi_refs: Vec<_> = incoming_vals.iter().zip(incoming_bbs.iter())
                    .map(|(v, bb)| (v as &dyn inkwell::values::BasicValue, *bb))
                    .collect();
                phi.add_incoming(&phi_refs);
                Ok(phi.as_basic_value())
            }
            Expr::Record { ty, fields } => {
                // Create a record value
                let type_name = ty.as_deref().unwrap_or("unknown");
                let llvm_ty = self.type_llvm.get(type_name)
                    .ok_or_else(|| format!("unknown type '{}'", type_name))?;
                if let BasicTypeEnum::StructType(sty) = llvm_ty {
                    let alloca = self.builder.build_alloca(*sty, type_name)
                        .map_err(|e| format!("alloca error: {}", e))?;
                    // Store field values
                    for (i, field) in fields.iter().enumerate() {
                        let val = self.compile_expr(&field.value, vars)?;
                        let gep = self.builder.build_struct_gep(*sty, alloca, i as u32, &field.name)
                            .map_err(|e| format!("gep error: {}", e))?;
                        self.builder.build_store(gep, val)
                            .map_err(|e| format!("store error: {}", e))?;
                    }
                    Ok(alloca.into())
                } else {
                    Err(format!("type '{}' is not a struct", type_name))
                }
            }
            Expr::Field(_obj, _field_name) => {
                // Field access: simplified for now - return i64 placeholder
                // TODO: Implement proper field access with GEP
                Ok(self.context.i64_type().const_int(0, false).into())
            }
            _ => Err(format!("unsupported expression in codegen: {:?}", expr)),
        }
    }

    fn compile_binop(
        &self,
        op: BinOp,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match op {
            BinOp::Add => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_add(l, r, "add").map_err(|e| format!("add error: {}", e))?.into()),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    Ok(self.builder.build_float_add(l, r, "fadd").map_err(|e| format!("add error: {}", e))?.into()),
                _ => Err("add requires same numeric types".into()),
            },
            BinOp::Sub => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_sub(l, r, "sub").map_err(|e| format!("sub error: {}", e))?.into()),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    Ok(self.builder.build_float_sub(l, r, "fsub").map_err(|e| format!("sub error: {}", e))?.into()),
                _ => Err("sub requires same numeric types".into()),
            },
            BinOp::Mul => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_mul(l, r, "mul").map_err(|e| format!("mul error: {}", e))?.into()),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    Ok(self.builder.build_float_mul(l, r, "fmul").map_err(|e| format!("mul error: {}", e))?.into()),
                _ => Err("mul requires same numeric types".into()),
            },
            BinOp::Div => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_signed_div(l, r, "div").map_err(|e| format!("div error: {}", e))?.into()),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    Ok(self.builder.build_float_div(l, r, "fdiv").map_err(|e| format!("div error: {}", e))?.into()),
                _ => Err("div requires same numeric types".into()),
            },
            BinOp::Mod => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_signed_rem(l, r, "rem").map_err(|e| format!("rem error: {}", e))?.into()),
                _ => Err("mod requires integer types".into()),
            },
            BinOp::EqCmp => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::EQ, l, r, "eq").map_err(|e| format!("cmp error: {}", e))?.into()),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    Ok(self.builder.build_float_compare(inkwell::FloatPredicate::OEQ, l, r, "feq").map_err(|e| format!("cmp error: {}", e))?.into()),
                _ => Err("eq requires same types".into()),
            },
            BinOp::NeCmp => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::NE, l, r, "ne").map_err(|e| format!("cmp error: {}", e))?.into()),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    Ok(self.builder.build_float_compare(inkwell::FloatPredicate::ONE, l, r, "fne").map_err(|e| format!("cmp error: {}", e))?.into()),
                _ => Err("ne requires same types".into()),
            },
            BinOp::Lt => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLT, l, r, "lt").map_err(|e| format!("cmp error: {}", e))?.into()),
                _ => Err("lt requires integer types".into()),
            },
            BinOp::Gt => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGT, l, r, "gt").map_err(|e| format!("cmp error: {}", e))?.into()),
                _ => Err("gt requires integer types".into()),
            },
            BinOp::Le => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLE, l, r, "le").map_err(|e| format!("cmp error: {}", e))?.into()),
                _ => Err("le requires integer types".into()),
            },
            BinOp::Ge => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGE, l, r, "ge").map_err(|e| format!("cmp error: {}", e))?.into()),
                _ => Err("ge requires integer types".into()),
            },
            BinOp::And => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_and(l, r, "and").map_err(|e| format!("and error: {}", e))?.into()),
                _ => Err("and requires boolean types".into()),
            },
            BinOp::Or => match (lhs, rhs) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    Ok(self.builder.build_or(l, r, "or").map_err(|e| format!("or error: {}", e))?.into()),
                _ => Err("or requires boolean types".into()),
            },
            _ => Err(format!("unsupported binary operator {:?}", op)),
        }
    }

    fn compile_call(
        &self,
        name: &str,
        args: &[Expr],
        vars: &HashMap<String, VarEntry<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let mut compiled_args = Vec::new();
        for arg in args {
            compiled_args.push(self.compile_expr(arg, vars)?);
        }

        let metadata_args: Vec<_> = compiled_args.into_iter().map(|v| {
            match v {
                BasicValueEnum::IntValue(iv) => inkwell::values::BasicMetadataValueEnum::IntValue(iv),
                BasicValueEnum::FloatValue(fv) => inkwell::values::BasicMetadataValueEnum::FloatValue(fv),
                BasicValueEnum::PointerValue(pv) => inkwell::values::BasicMetadataValueEnum::PointerValue(pv),
                BasicValueEnum::StructValue(sv) => inkwell::values::BasicMetadataValueEnum::StructValue(sv),
                BasicValueEnum::ArrayValue(av) => inkwell::values::BasicMetadataValueEnum::ArrayValue(av),
                BasicValueEnum::VectorValue(vv) => inkwell::values::BasicMetadataValueEnum::VectorValue(vv),
            }
        }).collect();

        if let Some(function) = self.module.get_function(name) {
            let call = self.builder.build_call(function, &metadata_args, "call")
                .map_err(|e| format!("call error: {}", e))?;
            Ok(call.try_as_basic_value().left().unwrap_or(
                self.context.i64_type().const_int(0, false).into()
            ))
        } else {
            Err(format!("undefined function '{}' in codegen", name))
        }
    }

    pub fn emit_ir(&self) -> String {
        self.module.print_to_string().to_string()
    }

    pub fn compile_to_object(&self, output_path: &Path) -> Result<(), String> {
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| format!("failed to initialize target: {}", e))?;
        let target = Target::from_name("x86-64")
            .ok_or("failed to find x86-64 target")?;
        let tm = target.create_target_machine(
            &TargetMachine::get_default_triple(),
            "x86-64",
            TargetMachine::get_host_cpu_features().to_string().as_str(),
            OptimizationLevel::Aggressive,
            RelocMode::Default,
            CodeModel::Default,
        ).ok_or("failed to create target machine")?;

        tm.write_to_file(&self.module, inkwell::targets::FileType::Object, output_path)
            .map_err(|e| format!("failed to write object file: {}", e))
    }
}
