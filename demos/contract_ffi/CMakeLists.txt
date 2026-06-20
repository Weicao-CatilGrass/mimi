cmake_minimum_required(VERSION 3.14)
project(contracts_simple_bindings LANGUAGES C CXX)

find_package(pybind11 REQUIRED)
pybind11_add_module(contracts_simple bindings.cpp)

target_link_libraries(contracts_simple PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/build/libcontracts.so
    ${CMAKE_CURRENT_SOURCE_DIR}/build/mimi_runtime.o
)
set_target_properties(contracts_simple PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
