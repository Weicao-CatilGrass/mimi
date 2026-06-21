use super::*;

impl<'a> Interpreter<'a> {
    // === Process control ===
    pub(crate) fn builtin_exit(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        let code = if args.is_empty() {
            0
        } else {
            match &args[0] {
                Value::Int(n) => *n as i32,
                _ => return Err(InterpError::new("exit expects an integer exit code")),
            }
        };
        std::process::exit(code)
    }
}
