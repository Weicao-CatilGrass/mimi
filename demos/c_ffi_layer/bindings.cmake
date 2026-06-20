cmake_minimum_required(VERSION 3.14)
project(strutil_bindings LANGUAGES CXX)

find_package(pybind11 REQUIRED)
find_library(MIMI_RUNTIME_LIB NAMES mimi_runtime PATHS "/usr/local/lib")

pybind11_add_module(strutil bindings.cpp)
target_include_directories(strutil PRIVATE "./")
target_link_libraries(strutil PRIVATE ${MIMI_RUNTIME_LIB})
find_library(MIMI_USER_LIB NAMES strutil PATHS "build")
target_link_libraries(strutil PRIVATE ${MIMI_USER_LIB})
set_target_properties(strutil PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
