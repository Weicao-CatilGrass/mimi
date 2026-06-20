cmake_minimum_required(VERSION 3.14)
project(strutil_bindings LANGUAGES C CXX)

find_package(pybind11 REQUIRED)
pybind11_add_module(strutil bindings.cpp)

target_include_directories(strutil PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(strutil PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/build/libstrutil_wrap.so
    ${CMAKE_CURRENT_SOURCE_DIR}/build/libstrutil.so
    ${CMAKE_CURRENT_SOURCE_DIR}/build/mimi_runtime.o
)
set_target_properties(strutil PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
