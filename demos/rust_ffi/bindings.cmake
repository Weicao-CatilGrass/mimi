cmake_minimum_required(VERSION 3.14)
project(rust_functions_bindings LANGUAGES CXX)

find_package(pybind11 REQUIRED)
find_library(MIMI_RUNTIME_LIB NAMES mimi_runtime PATHS "/usr/local/lib")

pybind11_add_module(rust_functions bindings.cpp)
target_include_directories(rust_functions PRIVATE "./")
target_link_libraries(rust_functions PRIVATE ${MIMI_RUNTIME_LIB})
find_library(MIMI_USER_LIB NAMES rust_functions PATHS "build")
target_link_libraries(rust_functions PRIVATE ${MIMI_USER_LIB})
set_target_properties(rust_functions PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
