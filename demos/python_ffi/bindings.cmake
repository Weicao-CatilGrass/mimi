cmake_minimum_required(VERSION 3.14)
project(math_bindings LANGUAGES CXX)

find_package(pybind11 REQUIRED)
find_library(MIMI_RUNTIME_LIB NAMES mimi_runtime PATHS "/usr/local/lib")

pybind11_add_module(math bindings.cpp)
target_include_directories(math PRIVATE "./")
target_link_libraries(math PRIVATE ${MIMI_RUNTIME_LIB})
find_library(MIMI_USER_LIB NAMES math PATHS "build")
target_link_libraries(math PRIVATE ${MIMI_USER_LIB})
set_target_properties(math PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)
