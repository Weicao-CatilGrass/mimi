MIMI = ../../target/debug/mimi
PYTHON = python3
MIMI_RUNTIME = ../../src/runtime/mimi_runtime.c

PYTHON_EXT = $(shell $(PYTHON) -c "import importlib.machinery; print(importlib.machinery.EXTENSION_SUFFIXES[0])")

all: check-deps build/contracts_simple$(PYTHON_EXT)

check-deps:
	@$(PYTHON) -c "import pybind11" 2>/dev/null || $(PYTHON) -m pip install pybind11 -q

# Step 1: Build Mimi shared library with contract verification (default behavior)
build/libcontracts.so: contracts_simple.mimi
	mkdir -p build
	$(MIMI) build --shared --verify-contracts $< -o $@

# Step 2: Generate C header
mimi_ffi.h: contracts_simple.mimi
	$(MIMI) emit-c-headers $< -o $@

# Step 3: Generate pybind11 bindings
bindings.cpp: contracts_simple.mimi
	$(MIMI) emit-py-bindings $< -o $@ --mimi-lib build/libcontracts.so

# Step 4: Compile Mimi runtime
build/mimi_runtime.o: $(MIMI_RUNTIME)
	mkdir -p build
	cc -fPIC -c $< -o $@

# Step 5: Build Python extension
build/contracts_simple$(PYTHON_EXT): bindings.cpp mimi_ffi.h build/libcontracts.so build/mimi_runtime.o
	cd build && cmake .. \
		-DPYTHON_EXECUTABLE=$(shell $(PYTHON) -c "import sys; print(sys.executable)") \
		-Dpybind11_DIR=$(shell $(PYTHON) -c "import pybind11; print(pybind11.get_cmake_dir())" 2>/dev/null || echo "") \
		2>&1 | tail -1
	cd build && cmake --build . 2>&1 | tail -3

.PHONY: test clean check-deps test_invalid

test: all
	PYTHONPATH=build $(PYTHON) test_contracts.py

# Invalid-input tests run in subprocess (contract violation calls abort())
test_invalid: all
	PYTHONPATH=build $(PYTHON) test_contracts_invalid.py

clean:
	rm -rf build mimi_ffi.h bindings.cpp bindings.cmake
