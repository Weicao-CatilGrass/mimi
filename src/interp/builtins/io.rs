use super::*;

impl<'a> Interpreter<'a> {
    // === I/O ===
    pub(crate) fn builtin_println(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        let parts: Vec<String> = args.iter().map(|v| v.to_string()).collect();
        println!("{}", parts.join(" "));
        Ok(Value::Unit)
    }

    pub(crate) fn builtin_print(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        let parts: Vec<String> = args.iter().map(|v| v.to_string()).collect();
        print!("{}", parts.join(" "));
        Ok(Value::Unit)
    }

    pub(crate) fn builtin_input(&mut self, _args: Vec<Value>) -> Result<Value, InterpError> {
        use std::io::{self, BufRead};
        let mut line = String::new();
        match io::stdin().lock().read_line(&mut line) {
            Ok(_) => {
                if line.ends_with('\n') { line.pop(); }
                if line.ends_with('\r') { line.pop(); }
                Ok(Value::Variant("Ok".into(), vec![Value::String(line)]))
            }
            Err(e) => Ok(Value::Variant("Err".into(), vec![Value::String(format!("input error: {}", e))])),
        }
    }

    // === Assertions ===
    pub(crate) fn builtin_assert(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new("assert expects 1 argument"));
        }
        if !is_truthy(&args[0]) {
            return Err(InterpError::new(format!("assertion failed: {}", args[0])));
        }
        Ok(Value::Unit)
    }

    pub(crate) fn builtin_assert_eq(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 2 {
            return Err(InterpError::new("assert_eq expects 2 arguments"));
        }
        if !values_equal(&args[0], &args[1]) {
            return Err(InterpError::new(format!("assertion failed: {} != {}", args[0], args[1])));
        }
        Ok(Value::Unit)
    }

    pub(crate) fn builtin_assert_ne(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 2 {
            return Err(InterpError::new("assert_ne expects 2 arguments"));
        }
        if values_equal(&args[0], &args[1]) {
            return Err(InterpError::new(format!("assertion failed: {} == {}", args[0], args[1])));
        }
        Ok(Value::Unit)
    }

    pub(crate) fn builtin_assert_approx_eq(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 2 {
            return Err(InterpError::new("assert_approx_eq expects 2 arguments"));
        }
        match (&args[0], &args[1]) {
            (Value::Float(a), Value::Float(b)) => {
                if (a - b).abs() > f64::EPSILON {
                    return Err(InterpError::new(format!("assertion failed: {} != {} (difference: {})", a, b, (a - b).abs())));
                }
                Ok(Value::Unit)
            }
            (Value::Int(a), Value::Int(b)) => {
                if a != b {
                    return Err(InterpError::new(format!("assertion failed: {} != {}", a, b)));
                }
                Ok(Value::Unit)
            }
            _ => {
                if !values_equal(&args[0], &args[1]) {
                    return Err(InterpError::new(format!("assertion failed: {} != {}", args[0], args[1])));
                }
                Ok(Value::Unit)
            }
        }
    }
    // === File I/O ===
    pub(crate) fn builtin_read_file(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("read_file expects 1 argument (path)")); }
        match &args[0] {
            Value::String(path) => {
                match std::fs::read_to_string(path) {
                    Ok(content) => Ok(Value::Variant("Ok".into(), vec![Value::String(content)])),
                    Err(e) => Ok(Value::Variant("Err".into(), vec![Value::String(format!("read_file error: {}", e))])),
                }
            }
            _ => Err(InterpError::new("read_file expects a string path")),
        }
    }

    pub(crate) fn builtin_write_file(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 2 { return Err(InterpError::new("write_file expects 2 arguments (path, content)")); }
        match (&args[0], &args[1]) {
            (Value::String(path), Value::String(content)) => {
                match std::fs::write(path, content) {
                    Ok(()) => Ok(Value::Variant("Ok".into(), vec![Value::Unit])),
                    Err(e) => Ok(Value::Variant("Err".into(), vec![Value::String(format!("write_file error: {}", e))])),
                }
            }
            _ => Err(InterpError::new("write_file expects (string, string)")),
        }
    }

    pub(crate) fn builtin_file_exists(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 { return Err(InterpError::new("file_exists expects 1 argument")); }
        match &args[0] {
            Value::String(path) => Ok(Value::Bool(std::path::Path::new(path).exists())),
            _ => Err(InterpError::new("file_exists expects a string path")),
        }
    }
    // === I/O (stderr) ===
    pub(crate) fn builtin_eprintln(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        let parts: Vec<String> = args.iter().map(|v| v.to_string()).collect();
        eprintln!("{}", parts.join(" "));
        Ok(Value::Unit)
    }
}
