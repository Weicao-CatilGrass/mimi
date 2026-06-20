use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Core implementation: fibonacci
fn do_rust_fib(n: i64) -> i64 {
    if n <= 0 { 0 }
    else if n == 1 { 1 }
    else { do_rust_fib(n - 1) + do_rust_fib(n - 2) }
}

/// Core implementation: sum of squares
fn do_rust_sum_squares(n: i64) -> i64 {
    (1..=n).map(|i| i * i).sum()
}

/// For Mimi codegen (uses __mimi_extern_ prefix)
#[no_mangle]
pub extern "C" fn __mimi_extern_rust_fib(n: i64) -> i64 { do_rust_fib(n) }

/// For Mimi codegen
#[no_mangle]
pub extern "C" fn __mimi_extern_rust_sum_squares(n: i64) -> i64 { do_rust_sum_squares(n) }

/// For C header / pybind consumers (plain name as declared in Mimi extern "C" block)
#[no_mangle]
pub extern "C" fn rust_fib(n: i64) -> i64 { do_rust_fib(n) }

/// For C header / pybind consumers
#[no_mangle]
pub extern "C" fn rust_sum_squares(n: i64) -> i64 { do_rust_sum_squares(n) }

/// Convert to uppercase (string manipulation)
#[no_mangle]
pub extern "C" fn __mimi_extern_rust_hello(name: *const c_char) -> *mut c_char {
    let c_str = unsafe { CStr::from_ptr(name) };
    let greeting = format!("Hello, {}!", c_str.to_str().unwrap_or("world"));
    CString::new(greeting).unwrap().into_raw()
}

/// Free a string returned by Rust
#[no_mangle]
pub extern "C" fn __mimi_extern_rust_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)); }
    }
}
