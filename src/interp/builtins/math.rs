use super::*;

impl<'a> Interpreter<'a> {
    // === Arithmetic ===
    pub(crate) fn builtin_sqrt(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new("sqrt expects 1 argument"));
        }
        match &args[0] {
            Value::Int(v) => Ok(Value::Float((*v as f64).sqrt())),
            Value::Float(v) => Ok(Value::Float(v.sqrt())),
            _ => Err(InterpError::new("sqrt expects a number")),
        }
    }

    pub(crate) fn builtin_abs(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new("abs expects 1 argument"));
        }
        match &args[0] {
            Value::Int(v) => Ok(Value::Int(v.abs())),
            Value::Float(v) => Ok(Value::Float(v.abs())),
            _ => Err(InterpError::new("abs expects a number")),
        }
    }

    pub(crate) fn builtin_pow(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 2 { return Err(InterpError::new("pow expects 2 arguments (base, exp)")); }
        match (&args[0], &args[1]) {
            (Value::Int(b), Value::Int(e)) => match b.checked_pow(*e as u32) { Some(v) => Ok(Value::Int(v)), None => Err(InterpError::new(format!("integer overflow in pow({}, {})", b, e))) },
            (Value::Float(b), Value::Int(e)) => Ok(Value::Float(b.powf(*e as f64))),
            (Value::Float(b), Value::Float(e)) => Ok(Value::Float(b.powf(*e))),
            _ => Err(InterpError::new("pow expects numbers")),
        }
    }

    pub(crate) fn builtin_floor(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("floor expects 1 argument")); }
        match &args[0] {
            Value::Float(v) => Ok(Value::Float(v.floor())),
            Value::Int(v) => Ok(Value::Int(*v)),
            _ => Err(InterpError::new("floor expects a number")),
        }
    }

    pub(crate) fn builtin_ceil(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("ceil expects 1 argument")); }
        match &args[0] {
            Value::Float(v) => Ok(Value::Float(v.ceil())),
            Value::Int(v) => Ok(Value::Int(*v)),
            _ => Err(InterpError::new("ceil expects a number")),
        }
    }

    pub(crate) fn builtin_round(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("round expects 1 argument")); }
        match &args[0] {
            Value::Float(v) => Ok(Value::Float(v.round())),
            Value::Int(v) => Ok(Value::Int(*v)),
            _ => Err(InterpError::new("round expects a number")),
        }
    }

    pub(crate) fn builtin_min(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 2 {
            return Err(InterpError::new("min expects 2 arguments"));
        }
        match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
            _ => Err(InterpError::new("min expects two numbers of the same type")),
        }
    }

    pub(crate) fn builtin_max(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 2 {
            return Err(InterpError::new("max expects 2 arguments"));
        }
        match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
            _ => Err(InterpError::new("max expects two numbers of the same type")),
        }
    }

    pub(crate) fn builtin_random(&self, _args: Vec<Value>) -> Result<Value, InterpError> {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        let s = RandomState::new();
        let mut hasher = s.build_hasher();
        hasher.write_u64(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64);
        let bits = hasher.finish();
        Ok(Value::Float((bits as f64) / (u64::MAX as f64)))
    }

    pub(crate) fn builtin_pi(&self, _args: Vec<Value>) -> Result<Value, InterpError> {
        Ok(Value::Float(std::f64::consts::PI))
    }
}
