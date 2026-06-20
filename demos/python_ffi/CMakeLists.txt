cmake_minimum_required(VERSION 3.14)
project(math_bindings LANGUAGES CXX)

find_package(pybind11 REQUIRED)
pybind11_add_module(math bindings.cpp)

target_include_directories(math PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(math PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/build/libmath.so
    ${CMAKE_CURRENT_SOURCE_DIR}/build/mimi_runtime.o
)
set_target_properties(math PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
