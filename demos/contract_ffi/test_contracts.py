"""Test: Python → Mimi with contract verification (requires clauses)."""
import sys
sys.path.insert(0, "build")

try:
    import contracts_simple
except ImportError as e:
    print(f"FAIL: could not import contracts_simple module: {e}")
    sys.exit(1)

# factorial: valid inputs (passes requires: n >= 0)
assert contracts_simple.factorial(0) == 1
assert contracts_simple.factorial(1) == 1
assert contracts_simple.factorial(5) == 120
assert contracts_simple.factorial(10) == 3628800

# divide_safe: valid inputs (passes requires: b != 0)
assert contracts_simple.divide_safe(10, 2) == 5
assert contracts_simple.divide_safe(7, 3) == 2
assert contracts_simple.divide_safe(-10, 2) == -5
assert contracts_simple.divide_safe(0, 5) == 0

print("All valid-input tests passed!")
print(f"  factorial(0)  = {contracts_simple.factorial(0)}")
print(f"  factorial(5)  = {contracts_simple.factorial(5)}")
print(f"  factorial(10) = {contracts_simple.factorial(10)}")
print(f"  divide_safe(10, 2)  = {contracts_simple.divide_safe(10, 2)}")
print(f"  divide_safe(7, 3)   = {contracts_simple.divide_safe(7, 3)}")
