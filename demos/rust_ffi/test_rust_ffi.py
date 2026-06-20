"""Test: Python → Mimi → Rust — three-layer FFI across three languages.

Pipeline: Python calls Mimi-exported functions (mimi_fib, mimi_sum_squares),
which internally call Rust functions via extern "C" declarations.
"""
import sys
sys.path.insert(0, "build")

try:
    import rust_functions
except ImportError as e:
    print(f"FAIL: could not import rust_functions module: {e}")
    sys.exit(1)

# mimi_fib wraps Rust's rust_fib
assert rust_functions.mimi_fib(0) == 0
assert rust_functions.mimi_fib(1) == 1
assert rust_functions.mimi_fib(10) == 55
assert rust_functions.mimi_fib(20) == 6765

# mimi_sum_squares wraps Rust's rust_sum_squares
assert rust_functions.mimi_sum_squares(1) == 1
assert rust_functions.mimi_sum_squares(3) == 14   # 1 + 4 + 9
assert rust_functions.mimi_sum_squares(5) == 55   # 1 + 4 + 9 + 16 + 25
assert rust_functions.mimi_sum_squares(10) == 385 # sum of squares 1..10

print("All tests passed!")
print(f"  mimi_fib(0)           = {rust_functions.mimi_fib(0)}")
print(f"  mimi_fib(10)          = {rust_functions.mimi_fib(10)}")
print(f"  mimi_fib(20)          = {rust_functions.mimi_fib(20)}")
print(f"  mimi_sum_squares(5)   = {rust_functions.mimi_sum_squares(5)}")
print(f"  mimi_sum_squares(10)  = {rust_functions.mimi_sum_squares(10)}")
