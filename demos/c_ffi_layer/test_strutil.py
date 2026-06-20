"""Test: Python → Mimi → C library — three-layer FFI."""
import sys
sys.path.insert(0, "build")

try:
    import strutil
except ImportError as e:
    print(f"FAIL: could not import strutil module: {e}")
    sys.exit(1)

assert strutil.reverse(12345) == 54321, f"reverse(12345) = {strutil.reverse(12345)}"
assert strutil.reverse(1) == 1, f"reverse(1) = {strutil.reverse(1)}"
assert strutil.reverse(0) == 0, f"reverse(0) = {strutil.reverse(0)}"
assert strutil.reverse(-123) == -321, f"reverse(-123) = {strutil.reverse(-123)}"

assert strutil.count_digits(12345) == 5
assert strutil.count_digits(0) == 1
assert strutil.count_digits(-987) == 3

assert strutil.is_palindrome(121) == True
assert strutil.is_palindrome(123) == False
assert strutil.is_palindrome(0) == True

assert strutil.gcd(12, 8) == 4
assert strutil.gcd(17, 5) == 1
assert strutil.gcd(0, 5) == 5

print("All tests passed!")
print(f"  reverse(12345)       = {strutil.reverse(12345)}")
print(f"  reverse(-123)        = {strutil.reverse(-123)}")
print(f"  count_digits(12345)  = {strutil.count_digits(12345)}")
print(f"  is_palindrome(121)   = {strutil.is_palindrome(121)}")
print(f"  is_palindrome(123)   = {strutil.is_palindrome(123)}")
print(f"  gcd(12, 8)           = {strutil.gcd(12, 8)}")
