use crate::ast::*;
use crate::codegen::types;
use crate::codegen::CodeGenerator;
use crate::error::{CompileError, MimiResult};
use inkwell::types::{BasicType, BasicTypeEnum};

impl<'ctx> CodeGenerator<'ctx> {
    pub(in crate::codegen) fn register_type_def(&mut self, t: &crate::ast::TypeDef) -> MimiResult<()> {
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
            crate::ast::TypeDefKind::Enum(variants) => {
                if t.attributes.contains(&TypeAttribute::ReprC) {
                    // #[repr(C)] enums are plain i32 (matching C int / enum)
                    let enum_ty = BasicTypeEnum::IntType(self.context.i32_type());
                    // Register constructor functions for each variant
                    for (ordinal, v) in variants.iter().enumerate() {
                        let ctor_name = format!("{}_{}", t.name, v.name);
                        if self.module.get_function(&ctor_name).is_none() {
                            let fn_type = self.context.i32_type().fn_type(&[], false);
                            let ctor = self.module.add_function(&ctor_name, fn_type, Some(inkwell::module::Linkage::Internal));
                            let entry = self.context.append_basic_block(ctor, "entry");
                            let prev_block = self.builder.get_insert_block();
                            self.builder.position_at_end(entry);
                            self.builder.build_return(Some(&self.context.i32_type().const_int(ordinal as u64, false)))
                                .map_err(|e| CompileError::LlvmError(format!("ctor return error: {}", e)))?;
                            if let Some(prev) = prev_block { self.builder.position_at_end(prev); }
                        }
                    }
                    enum_ty
                } else {
                    // Internal enum representation: i32 tag + i64 payload
                    let tag_ty = BasicTypeEnum::IntType(self.context.i32_type());
                    let payload_ty = BasicTypeEnum::IntType(self.context.i64_type());
                    let enum_ty = BasicTypeEnum::StructType(self.context.struct_type(&[tag_ty, payload_ty], false));
                    // Register constructor functions for each variant
                    let struct_ty = self.context.struct_type(&[
                        BasicTypeEnum::IntType(self.context.i32_type()),
                        BasicTypeEnum::IntType(self.context.i64_type()),
                    ], false);
                    for (ordinal, v) in variants.iter().enumerate() {
                        let ctor_name = format!("{}_{}", t.name, v.name);
                        if self.module.get_function(&ctor_name).is_none() {
                            let fn_type = struct_ty.fn_type(&[
                                inkwell::types::BasicMetadataTypeEnum::IntType(self.context.i64_type()),
                            ], false);
                            let ctor = self.module.add_function(&ctor_name, fn_type, Some(inkwell::module::Linkage::Internal));
                            let entry = self.context.append_basic_block(ctor, "entry");
                            let prev_block = self.builder.get_insert_block();
                            self.builder.position_at_end(entry);
                            let payload = ctor.get_nth_param(0).ok_or_else(|| CompileError::LlvmError("missing payload param".to_string()))?;
                            let alloca = self.builder.build_alloca(struct_ty, &ctor_name)
                                .map_err(|e| CompileError::LlvmError(format!("alloca error: {}", e)))?;
                            let tag_gep = self.builder.build_struct_gep(struct_ty, alloca, 0, "tag")
                                .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                            self.builder.build_store(tag_gep, self.context.i32_type().const_int(ordinal as u64, false))
                                .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                            let payload_gep = self.builder.build_struct_gep(struct_ty, alloca, 1, "payload")
                                .map_err(|e| CompileError::LlvmError(format!("gep error: {}", e)))?;
                            self.builder.build_store(payload_gep, payload)
                                .map_err(|e| CompileError::LlvmError(format!("store error: {}", e)))?;
                            let loaded = self.builder.build_load(struct_ty, alloca, &ctor_name)
                                .map_err(|e| CompileError::LlvmError(format!("load error: {}", e)))?;
                            self.builder.build_return(Some(&loaded))
                                .map_err(|e| CompileError::LlvmError(format!("return error: {}", e)))?;
                            if let Some(prev) = prev_block { self.builder.position_at_end(prev); }
                        }
                    }
                    enum_ty
                }
            }
            crate::ast::TypeDefKind::Alias(ty) | crate::ast::TypeDefKind::Newtype(ty) => {
                types::mimi_type_to_llvm(self.context, ty)
                    .unwrap_or(BasicTypeEnum::IntType(self.context.i64_type()))
            }
            crate::ast::TypeDefKind::Union(fields) => {
                // Represent union as a byte array large enough to hold the largest field
                let max_size = fields.iter().map(|f| {
                    let llvm_ty = types::mimi_type_to_llvm(self.context, &f.ty)
                        .unwrap_or(BasicTypeEnum::IntType(self.context.i64_type()));
                    llvm_ty.size_of()
                        .and_then(|s| s.get_zero_extended_constant())
                        .unwrap_or(8)
                }).max().unwrap_or(8);
                let array_ty = self.context.i8_type().array_type(max_size as u32);
                BasicTypeEnum::ArrayType(array_ty)
            }
        };
        self.type_llvm.insert(t.name.clone(), llvm_ty);
        self.type_defs.insert(t.name.clone(), t.clone());
        // Track record types for FFI serialization
        if matches!(t.kind, crate::ast::TypeDefKind::Record(_)) {
            self.record_type_names.insert(t.name.clone());
            if t.attributes.contains(&TypeAttribute::ReprC) {
                self.repr_c_record_names.insert(t.name.clone());
            }
        }
        Ok(())
    }


    pub(in crate::codegen) fn register_actor_def(&mut self, actor: &crate::ast::ActorDef) -> MimiResult<()> {
        // Represent actor as a struct with fields
        let mut field_tys = Vec::new();
        for f in &actor.fields {
            let ty = types::mimi_type_to_llvm(self.context, &f.ty)
                .unwrap_or(BasicTypeEnum::IntType(self.context.i64_type()));
            field_tys.push(ty);
        }
        let llvm_ty = BasicTypeEnum::StructType(self.context.struct_type(&field_tys, false));
        self.type_llvm.insert(actor.name.clone(), llvm_ty);
        
        // Also register as a type definition for field access
        let type_def = crate::ast::TypeDef {
            name: actor.name.clone(),
            commitment: actor.commitment,
            pub_: actor.pub_,
            kind: crate::ast::TypeDefKind::Record(actor.fields.iter().map(|f| crate::ast::Field {
                name: f.name.clone(),
                ty: f.ty.clone(),
            }).collect()),
            generics: Vec::new(),
            derives: Vec::new(),
            attributes: Vec::new(),
        };
        self.type_defs.insert(actor.name.clone(), type_def);
        Ok(())
    }
}
