"""Test that Mimi-exported functions are callable from Python via pybind11."""
import sys
sys.path.insert(0, "build")

try:
    import math  # the pybind11 module named "math"
except ImportError as e:
    print(f"FAIL: could not import math module: {e}")
    print("Run 'make' first to build the Python extension.")
    sys.exit(1)

assert math.add(3, 4) == 7, f"add(3, 4) should be 7, got {math.add(3, 4)}"
assert math.sub(10, 3) == 7, f"sub(10, 3) should be 7, got {math.sub(10, 3)}"
assert math.mul(6, 7) == 42, f"mul(6, 7) should be 42, got {math.mul(6, 7)}"
assert math.fact(5) == 120, f"fact(5) should be 120, got {math.fact(5)}"
assert math.fact(0) == 1, f"fact(0) should be 1, got {math.fact(0)}"

print("All tests passed!")
print(f"  add(3, 4)  = {math.add(3, 4)}")
print(f"  sub(10, 3) = {math.sub(10, 3)}")
print(f"  mul(6, 7)  = {math.mul(6, 7)}")
print(f"  fact(5)    = {math.fact(5)}")
print(f"  fact(0)    = {math.fact(0)}")
