MIMI = ../../target/debug/mimi
PYTHON = python3
MIMI_RUNTIME = ../../src/runtime/mimi_runtime.c
PYTHON_EXT = $(shell $(PYTHON) -c "import importlib.machinery; print(importlib.machinery.EXTENSION_SUFFIXES[0])")

all: check-deps build/rust_functions$(PYTHON_EXT)

check-deps:
	@$(PYTHON) -c "import pybind11" 2>/dev/null || $(PYTHON) -m pip install pybind11 -q

# Step 1: Build Rust FFI library
target/release/libmimi_rust_ffi.so: src/lib.rs Cargo.toml
	cargo build --release

# Step 2: Copy Rust lib to build dir
build/libmimi_rust_ffi.so: target/release/libmimi_rust_ffi.so
	mkdir -p build
	cp $< $@

# Step 3: Generate C header
mimi_ffi.h: rust_functions.mimi
	$(MIMI) emit-c-headers $< -o $@

# Step 4: Generate pybind11 bindings
bindings.cpp: rust_functions.mimi
	$(MIMI) emit-py-bindings $< -o $@ --mimi-lib build/librust_functions.so

# Step 5: Build Mimi shared library
build/librust_functions.so: rust_functions.mimi
	mkdir -p build
	$(MIMI) build --shared $< -o $@

# Step 6: Compile Mimi runtime
build/mimi_runtime.o: $(MIMI_RUNTIME)
	mkdir -p build
	cc -fPIC -c $< -o $@

# Step 7: Build Python extension
build/rust_functions$(PYTHON_EXT): bindings.cpp mimi_ffi.h build/librust_functions.so build/libmimi_rust_ffi.so build/mimi_runtime.o
	cd build && cmake .. \
		-DPYTHON_EXECUTABLE=$(shell $(PYTHON) -c "import sys; print(sys.executable)") \
		-Dpybind11_DIR=$(shell $(PYTHON) -c "import pybind11; print(pybind11.get_cmake_dir())" 2>/dev/null || echo "") \
		2>&1 | tail -1
	cd build && cmake --build . 2>&1 | tail -3

.PHONY: test clean check-deps

test: all
	PYTHONPATH=build $(PYTHON) test_rust_ffi.py

clean:
	rm -rf build mimi_ffi.h bindings.cpp bindings.cmake
	cargo clean
