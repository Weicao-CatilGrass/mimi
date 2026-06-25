use super::*;

impl<'a> Interpreter<'a> {
    pub(crate) fn builtin_type_name(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new("type_name expects 1 argument (a value)"));
        }
        let type_name = self.value_type_name(&args[0]);
        Ok(Value::String(type_name))
    }

    /// Extract a string type name from a Value (either Value::String or Value::Type).
    fn resolve_type_name_arg<'v>(&self, v: &'v Value) -> Result<&'v String, InterpError> {
        match v {
            Value::String(name) => Ok(name),
            Value::Type(name) => Ok(name),
            _ => Err(InterpError::new(
                "expected a type name string or Type value",
            )),
        }
    }

    pub(crate) fn builtin_type_fields(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new(
                "type_fields expects 1 argument (a type name string)",
            ));
        }
        let name = self.resolve_type_name_arg(&args[0])?;
        let type_def = self
            .type_defs
            .get(name)
            .ok_or_else(|| InterpError::new(format!("unknown type '{}'", name)))?;
        match &type_def.kind {
            TypeDefKind::Record(fields) => {
                let field_names: Vec<Value> = fields
                    .iter()
                    .map(|f| Value::String(f.name.clone()))
                    .collect();
                Ok(Value::List(field_names))
            }
            TypeDefKind::Enum(variants) => {
                let variant_names: Vec<Value> = variants
                    .iter()
                    .map(|v| Value::String(v.name.clone()))
                    .collect();
                Ok(Value::List(variant_names))
            }
            _ => Ok(Value::List(vec![])),
        }
    }

    pub(crate) fn builtin_type_variants(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new(
                "type_variants expects 1 argument (a type name string)",
            ));
        }
        let name = self.resolve_type_name_arg(&args[0])?;
        let type_def = self
            .type_defs
            .get(name)
            .ok_or_else(|| InterpError::new(format!("unknown type '{}'", name)))?;
        match &type_def.kind {
            TypeDefKind::Enum(variants) => {
                let variant_names: Vec<Value> = variants
                    .iter()
                    .map(|v| Value::String(v.name.clone()))
                    .collect();
                Ok(Value::List(variant_names))
            }
            _ => Ok(Value::List(vec![])),
        }
    }
    // === Meta ===
    pub(crate) fn builtin_ast_dump(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new(
                "ast_dump expects 1 argument (a quoted AST)",
            ));
        }
        match &args[0] {
            Value::QuoteAst(q) => Ok(Value::String(format!("{:?}", q))),
            other => Ok(Value::String(format!("Not a QuoteAst: {}", other))),
        }
    }

    pub(crate) fn builtin_ast_eval(&mut self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new(
                "ast_eval expects 1 argument (a quoted AST)",
            ));
        }
        match &args[0] {
            Value::QuoteAst(q) => self.eval_quoted_ast(q),
            other => Err(InterpError::new(format!(
                "ast_eval expects a QuoteAst, got {}",
                other
            ))),
        }
    }
}
