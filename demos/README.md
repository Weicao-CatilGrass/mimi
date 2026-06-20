# Mimi Language Demos

This directory contains end-to-end FFI demos demonstrating the Mimi language
Python bridge pipeline:

```
Python → pybind11 → C wrapper → Mimi (.so) → (optional) extern "C" layer → C/Rust/...
```

## Prerequisites

- **pybind11**: `pip install pybind11`
- **Python 3**: with development headers
- **C compiler** (gcc/clang): for Mimi runtime and C FFI
- **Rust** (optional, for Rust FFI demo): `cargo`

## Demos

### 1. Basic Python FFI (`python_ffi/`)
Python → Mimi math functions (add, sub, mul, factorial).
No external C/Rust dependency. Simplest demo.

```
cd python_ffi && make test
```

### 2. C FFI Layer (`c_ffi_layer/`)
Python → Mimi → C library (strutil: reverse, count_digits, is_palindrome, gcd).
Demonstrates Mimi's `extern "C" { func ... }` import mechanism calling into a
custom C shared library.

```
cd c_ffi_layer && make test
```

### 3. Contract Verification (`contract_ffi/`)
Python → Mimi with `requires` runtime assertions.
Valid inputs pass; invalid inputs trigger `abort()` with diagnostic message.
Demonstrates `--verify-contracts` flag at build time.

```
cd contract_ffi && make test          # valid inputs
cd contract_ffi && make test_invalid  # contract violations → abort
```

### 4. Rust FFI (`rust_ffi/`)
Python → Mimi → Rust library (fibonacci, sum of squares).
Demonstrates three-language FFI using Rust's `extern "C"` ABI.

```
cd rust_ffi && make test
```

## Pipeline Steps

Each demo follows the same build pipeline:

1. `mimi build --shared` → compiled Mimi `.so`
2. `mimi emit-c-headers` → `mimi_ffi.h` (C API declarations)
3. `mimi emit-py-bindings --mimi-lib <path>` → `bindings.cpp` (pybind11 wrappers)
4. `cmake` + `cmake --build` → Python extension (`.cpython-*.so`)
