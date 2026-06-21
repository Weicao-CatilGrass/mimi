use super::*;

impl<'a> Interpreter<'a> {
    // === MimiSpec runtime ===
    pub(crate) fn builtin_lexer(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("lexer expects 1 argument (source string)")); }
        match &args[0] {
            Value::String(source) => {
                match mimispec::tokenize(source) {
                    Ok(tokens) => {
                        let token_values: Vec<Value> = tokens.iter().map(|t| {
                            Value::Record(None, {
                                let mut fields = std::collections::HashMap::new();
                                fields.insert("kind".into(), Value::String(format!("{:?}", t.kind)));
                                fields.insert("line".into(), Value::Int(t.line as i64));
                                fields.insert("col".into(), Value::Int(t.col as i64));
                                fields
                            })
                        }).collect();
                        Ok(Value::List(token_values))
                    }
                    Err(e) => Err(InterpError::new(format!("lexer error: {}", e))),
                }
            }
            _ => Err(InterpError::new("lexer expects a string source")),
        }
    }

    pub(crate) fn builtin_parse(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("parse expects 1 argument (source string)")); }
        match &args[0] {
            Value::String(source) => {
                let result = mimispec::parse(source);
                if result.errors.is_empty() {
                    // Convert AST to a simple record representation
                    let mut fields = std::collections::HashMap::new();
                    fields.insert("imports".into(), Value::List(vec![]));
                    fields.insert("rules".into(), Value::List(vec![]));
                    fields.insert("fragments".into(), Value::List(vec![]));
                    Ok(Value::Record(Some("MmsAst".into()), fields))
                } else {
                    let errors: Vec<Value> = result.errors.iter().map(|e| {
                        Value::Record(None, {
                            let mut fields = std::collections::HashMap::new();
                            fields.insert("message".into(), Value::String(e.to_string()));
                            fields.insert("line".into(), Value::Int(e.line as i64));
                            fields.insert("col".into(), Value::Int(e.col as i64));
                            fields
                        })
                    }).collect();
                    Ok(Value::Tuple(vec![Value::Bool(false), Value::List(errors)]))
                }
            }
            _ => Err(InterpError::new("parse expects a string source")),
        }
    }
}
