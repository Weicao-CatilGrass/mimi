use crate::ast::Type;
use inkwell::context::Context;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::AddressSpace;

pub fn mimi_type_to_llvm<'ctx>(ctx: &'ctx Context, ty: &Type) -> Option<BasicTypeEnum<'ctx>> {
    match ty {
        Type::Name(name, _) => match name.as_str() {
            "i32" | "i64" => Some(BasicTypeEnum::IntType(ctx.i64_type())),
            "f64" => Some(BasicTypeEnum::FloatType(ctx.f64_type())),
            "bool" => Some(BasicTypeEnum::IntType(ctx.bool_type())),
            "string" => {
                let i8_ptr = ctx.i8_type().ptr_type(AddressSpace::default());
                let i64 = ctx.i64_type();
                let fields = [BasicTypeEnum::PointerType(i8_ptr), BasicTypeEnum::IntType(i64)];
                Some(BasicTypeEnum::StructType(ctx.struct_type(&fields, false)))
            }
            "unit" | "nothing" => None,
            _ => Some(BasicTypeEnum::IntType(ctx.i64_type())),
        },
        Type::Ref(inner) | Type::RefMut(inner) => {
            let inner_llvm = mimi_type_to_llvm(ctx, inner)?;
            let ptr = match inner_llvm {
                BasicTypeEnum::IntType(t) => BasicTypeEnum::PointerType(t.ptr_type(AddressSpace::default())),
                BasicTypeEnum::FloatType(t) => BasicTypeEnum::PointerType(t.ptr_type(AddressSpace::default())),
                BasicTypeEnum::PointerType(t) => BasicTypeEnum::PointerType(t.ptr_type(AddressSpace::default())),
                BasicTypeEnum::StructType(t) => BasicTypeEnum::PointerType(t.ptr_type(AddressSpace::default())),
                BasicTypeEnum::ArrayType(t) => BasicTypeEnum::PointerType(t.ptr_type(AddressSpace::default())),
                _ => BasicTypeEnum::PointerType(ctx.i8_type().ptr_type(AddressSpace::default())),
            };
            Some(ptr)
        }
        Type::Tuple(elems) => {
            let mut llvm_elems = Vec::new();
            for e in elems {
                llvm_elems.push(mimi_type_to_llvm(ctx, e)?);
            }
            Some(BasicTypeEnum::StructType(ctx.struct_type(&llvm_elems, false)))
        }
        Type::Shared(_) | Type::LocalShared(_) | Type::Weak(_) =>
            Some(BasicTypeEnum::PointerType(ctx.i8_type().ptr_type(AddressSpace::default()))),
        Type::Cap(_) => Some(BasicTypeEnum::IntType(ctx.i64_type())),
        Type::Newtype(_, inner) => mimi_type_to_llvm(ctx, inner),
        Type::Allocator => Some(BasicTypeEnum::IntType(ctx.i64_type())),
        _ => Some(BasicTypeEnum::IntType(ctx.i64_type())),
    }
}

pub fn basic_to_metadata<'ctx>(ctx: &'ctx Context, ty: BasicTypeEnum<'ctx>) -> BasicMetadataTypeEnum<'ctx> {
    match ty {
        BasicTypeEnum::IntType(t) => BasicMetadataTypeEnum::IntType(t),
        BasicTypeEnum::FloatType(t) => BasicMetadataTypeEnum::FloatType(t),
        BasicTypeEnum::PointerType(t) => BasicMetadataTypeEnum::PointerType(t),
        BasicTypeEnum::StructType(t) => BasicMetadataTypeEnum::StructType(t),
        BasicTypeEnum::ArrayType(t) => BasicMetadataTypeEnum::ArrayType(t),
        _ => BasicMetadataTypeEnum::IntType(ctx.i64_type()),
    }
}
