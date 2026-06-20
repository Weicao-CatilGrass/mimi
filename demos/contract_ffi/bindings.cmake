cmake_minimum_required(VERSION 3.14)
project(contracts_simple_bindings LANGUAGES CXX)

find_package(pybind11 REQUIRED)
find_library(MIMI_RUNTIME_LIB NAMES mimi_runtime PATHS "/usr/local/lib")

pybind11_add_module(contracts_simple bindings.cpp)
target_include_directories(contracts_simple PRIVATE "./")
target_link_libraries(contracts_simple PRIVATE ${MIMI_RUNTIME_LIB})
find_library(MIMI_USER_LIB NAMES contracts PATHS "build")
target_link_libraries(contracts_simple PRIVATE ${MIMI_USER_LIB})
set_target_properties(contracts_simple PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
