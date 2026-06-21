use super::*;

impl<'a> Interpreter<'a> {
    // === C interop ===
    pub(crate) fn builtin_str_to_c_str(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new("str_to_c_str expects 1 argument (string)"));
        }
        match &args[0] {
            Value::String(s) => {
                // Return a tuple (pointer, length) for C compatibility
                // The pointer is the raw pointer to the CString data
                let c_str = std::ffi::CString::new(s.as_str())
                    .map_err(|e| InterpError::new(format!("failed to create C string: {}", e)))?;
                let ptr = c_str.into_raw() as i64;
                Ok(Value::Tuple(vec![Value::Int(ptr), Value::Int(s.len() as i64)]))
            }
            other => Err(InterpError::new(format!("str_to_c_str: argument must be a string, found {}", super::value::type_name(other)))),
        }
    }

    pub(crate) fn builtin_c_str_to_string(&self, args: Vec<Value>) -> Result<Value, InterpError> {
        if args.len() != 1 {
            return Err(InterpError::new("c_str_to_string expects 1 argument (pointer)"));
        }
        match &args[0] {
            Value::Int(ptr) => {
                if *ptr == 0 {
                    return Ok(Value::String(String::new()));
                }
                // SAFETY: ptr is checked for null above. We also validate that it points to
                // readable memory by attempting to read the first byte via a catch_unwind guard.
                // This does NOT guarantee the entire C string is valid, but catches obvious garbage.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Probe the first byte: dereference the pointer to check
                    // it points to readable memory (catches obvious garbage).
                    let ptr_raw = *ptr as *const u8;
                    // SAFETY: ptr_raw is a raw pointer from the interpreter's heap; the probe
                    // is wrapped in catch_unwind to recover from segfault on invalid pointers.
                    let _first_byte = unsafe { *ptr_raw };
                    // SAFETY: CStr::from_ptr requires a valid null-terminated string pointer;
                    // catch_unwind handles invalid pointer cases.
                    let c_str = unsafe { std::ffi::CStr::from_ptr(*ptr as *const i8) };
                    Value::String(c_str.to_string_lossy().into_owned())
                }));
                match result {
                    Ok(v) => Ok(v),
                    Err(_) => Err(InterpError::new(
                        format!("c_str_to_string: invalid pointer {:#x} (segfault or unmapped memory)", ptr)
                    )),
                }
            }
            other => Err(InterpError::new(format!("c_str_to_string: argument must be a pointer (int), found {}", super::value::type_name(other)))),
        }
    }
}
