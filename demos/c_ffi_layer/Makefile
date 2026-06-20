MIMI = ../../target/debug/mimi
PYTHON = python3
MIMI_RUNTIME = ../../src/runtime/mimi_runtime.c

all: check-deps build/libstrutil.so build/strutil$(shell $(PYTHON) -c "import importlib.machinery; print(importlib.machinery.EXTENSION_SUFFIXES[0])")

check-deps:
	@$(PYTHON) -c "import pybind11" 2>/dev/null || $(PYTHON) -m pip install pybind11 -q

# Step 1: Build C library
build/libstrutil.so: libstrutil.c
	mkdir -p build
	cc -shared -fPIC -o $@ $<

# Step 2: Generate C header
mimi_ffi.h: strutil.mimi
	$(MIMI) emit-c-headers $< -o $@

# Step 3: Generate pybind11 bindings
bindings.cpp: strutil.mimi
	$(MIMI) emit-py-bindings $< -o $@ --mimi-lib build/libstrutil.so

# Step 4: Build Mimi shared library (links against C lib at runtime via undefined symbols)
build/libstrutil_wrap.so: strutil.mimi
	mkdir -p build
	$(MIMI) build --shared $< -o $@

# Step 5: Compile Mimi runtime
build/mimi_runtime.o: $(MIMI_RUNTIME)
	mkdir -p build
	cc -fPIC -c $< -o $@

# Step 6: Build Python extension (resolves C symbols via libstrutil.so)
build/strutil$(shell $(PYTHON) -c "import importlib.machinery; print(importlib.machinery.EXTENSION_SUFFIXES[0])"): bindings.cpp mimi_ffi.h build/libstrutil_wrap.so build/libstrutil.so build/mimi_runtime.o
	cd build && cmake .. \
		-DPYTHON_EXECUTABLE=$(shell $(PYTHON) -c "import sys; print(sys.executable)") \
		-Dpybind11_DIR=$(shell $(PYTHON) -c "import pybind11; print(pybind11.get_cmake_dir())" 2>/dev/null || echo "") \
		2>&1 | tail -1
	cd build && cmake --build . 2>&1 | tail -3

.PHONY: test clean check-deps

test: all
	PYTHONPATH=build $(PYTHON) test_strutil.py

clean:
	rm -rf build mimi_ffi.h bindings.cpp bindings.cmake
