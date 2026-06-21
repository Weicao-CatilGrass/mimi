use crate::ast::{Lit, Type};
use crate::core::checker::Checker;

impl<'a> Checker<'a> {
    pub(in crate::core) fn infer_literal(&self, l: &Lit) -> Type {
        match l {
            Lit::Int(_) => Type::Name("i32".into(), vec![]),
            Lit::Float(_) => Type::Name("f64".into(), vec![]),
            Lit::Bool(_) => Type::Name("bool".into(), vec![]),
            Lit::String(_) => Type::Name("string".into(), vec![]),
            Lit::FString(_) => Type::Name("string".into(), vec![]),
            Lit::Unit => Type::Name("unit".into(), vec![]),
        }
    }
}
