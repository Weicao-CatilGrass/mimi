cmake_minimum_required(VERSION 3.14)
project(rust_functions_bindings LANGUAGES C CXX)

find_package(pybind11 REQUIRED)
pybind11_add_module(rust_functions bindings.cpp)

target_link_libraries(rust_functions PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/build/librust_functions.so
    ${CMAKE_CURRENT_SOURCE_DIR}/build/libmimi_rust_ffi.so
    ${CMAKE_CURRENT_SOURCE_DIR}/build/mimi_runtime.o
)
set_target_properties(rust_functions PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
