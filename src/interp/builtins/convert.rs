use super::*;

impl<'a> Interpreter<'a> {
    pub(crate) fn builtin_to_int(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("to_int expects 1 argument")); }
        match &args[0] {
            Value::Int(v) => Ok(Value::Int(*v)),
            Value::Float(v) => Ok(Value::Int(*v as i64)),
            Value::String(s) => s.parse::<i64>()
                .map(Value::Int)
                .map_err(|e| InterpError::new(format!("to_int parse error: {}", e))),
            Value::Bool(b) => Ok(Value::Int(*b as i64)),
            _ => Err(InterpError::new("to_int cannot convert this type")),
        }
    }

    pub(crate) fn builtin_to_float(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("to_float expects 1 argument")); }
        match &args[0] {
            Value::Float(v) => Ok(Value::Float(*v)),
            Value::Int(v) => Ok(Value::Float(*v as f64)),
            Value::String(s) => s.parse::<f64>()
                .map(Value::Float)
                .map_err(|e| InterpError::new(format!("to_float parse error: {}", e))),
            _ => Err(InterpError::new("to_float cannot convert this type")),
        }
    }
    pub(crate) fn builtin_from_int(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.is_empty() {
            return Err(InterpError::new("from_int expects at least 1 argument (int)"));
        }
        match &args[0] {
            Value::Int(n) => Ok(Value::Int(*n)),
            _ => Err(InterpError::new("from_int: first arg must be an integer")),
        }
    }
}
