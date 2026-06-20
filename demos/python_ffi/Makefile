MIMI = ../../target/debug/mimi
PYTHON = python3
MIMI_RUNTIME = ../../src/runtime/mimi_runtime.c

# Auto-install pybind11 if missing
PIP_INSTALL := $(shell $(PYTHON) -c "import pybind11" 2>/dev/null || $(PYTHON) -m pip install pybind11 -q 2>&1)

all: check-deps build/math.so build/math$(shell $(PYTHON) -c "import importlib.machinery; print(importlib.machinery.EXTENSION_SUFFIXES[0])")

check-deps:
	@$(PYTHON) -c "import pybind11" 2>/dev/null || (echo "Installing pybind11..."; $(PYTHON) -m pip install pybind11 -q)

mimi_ffi.h: math.mimi
	$(MIMI) emit-c-headers $< -o $@

bindings.cpp: math.mimi
	$(MIMI) emit-py-bindings $< -o $@ --mimi-lib build/libmath.so

build/libmath.so: math.mimi
	mkdir -p build
	$(MIMI) build --shared $< -o $@

build/mimi_runtime.o: $(MIMI_RUNTIME)
	mkdir -p build
	cc -fPIC -c $< -o $@

# Build Python extension — pybind11 module name matches the .mimi file stem
build/math$(shell $(PYTHON) -c "import importlib.machinery; print(importlib.machinery.EXTENSION_SUFFIXES[0])"): bindings.cpp mimi_ffi.h build/libmath.so build/mimi_runtime.o
	cd build && cmake .. \
		-DPYTHON_EXECUTABLE=$(shell $(PYTHON) -c "import sys; print(sys.executable)") \
		-Dpybind11_DIR=$(shell $(PYTHON) -c "import pybind11; print(pybind11.get_cmake_dir())" 2>/dev/null || echo "") \
		2>&1 | tail -1
	cd build && cmake --build . 2>&1 | tail -3

.PHONY: test clean check-deps

test: all
	PYTHONPATH=build $(PYTHON) test_math.py

clean:
	rm -rf build mimi_ffi.h bindings.cpp bindings.cmake
