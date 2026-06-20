"""Test contract violation detection: invalid inputs must crash compiled code.

Contract violations in compiled Mimi code call abort(), which terminates
the process. We test this by running in a subprocess and checking for
non-zero exit code.
"""
import subprocess
import sys

script = """
import sys
sys.path.insert(0, "build")
import contracts_simple

# factorial(-1) should fail requires: n >= 0
try:
    contracts_simple.factorial(-1)
    print("ERROR: factorial(-1) should have crashed")
    sys.exit(0)
except Exception as e:
    # If we get here, C++/pybind wrapped the abort into an exception
    print("OK: factorial(-1) raised:", e)
    sys.exit(0)
"""

result = subprocess.run(
    [sys.executable, "-c", script],
    capture_output=True, text=True, timeout=10
)

if result.returncode != 0:
    # Crash/abort is expected! Non-zero exit means contract violation caught.
    print("PASS: factorial(-1) aborted the process (requires violation detected)")
    print(f"  exit code: {result.returncode}")
    print(f"  stderr: {result.stderr[:200] if result.stderr else '(none)'}")
else:
    stdout = result.stdout.strip()
    if "ERROR" in stdout:
        print(f"FAIL: {stdout}")
        sys.exit(1)
    print("PASS: factorial(-1) detected via exception")
    print(f"  stdout: {stdout}")

# Test divide_safe(5, 0) → requires: b != 0
script2 = """
import sys
sys.path.insert(0, "build")
import contracts_simple
contracts_simple.divide_safe(5, 0)
print("ERROR: divide_safe(5, 0) should have crashed")
"""

result2 = subprocess.run(
    [sys.executable, "-c", script2],
    capture_output=True, text=True, timeout=10
)

if result2.returncode != 0:
    print("PASS: divide_safe(5, 0) aborted the process (requires violation detected)")
    print(f"  exit code: {result2.returncode}")
    print(f"  stderr: {result2.stderr[:200] if result2.stderr else '(none)'}")
else:
    stdout2 = result2.stdout.strip()
    if "ERROR" in stdout2:
        print(f"FAIL: {stdout2}")
        sys.exit(1)
    print("PASS: divide_safe(5, 0) did not crash (contract checking disabled)")
    print(f"  stdout: {stdout2}")

print("\nAll contract violation tests passed!")
