#ifndef MIMI_FFI_H
#define MIMI_FFI_H

#include <stdint.h>
#include <stdbool.h>

typedef int64_t MimiHandle;

MimiHandle mimi_shared_retain(MimiHandle handle);
void mimi_shared_release(MimiHandle handle);
void* mimi_shared_get_ptr(MimiHandle handle);

// Capability API
typedef int64_t MimiCap;
bool mimi_cap_check(MimiCap cap, const char* name);
bool mimi_cap_consume(MimiCap cap, const char* name);

// String API
const char* mimi_string_as_c_str(void* mimi_string);
char* mimi_string_into_raw(void* mimi_string);
void* mimi_string_from_raw(char* c_str);
void mimi_string_free_raw(char* c_str);

// Type definitions

// Function declarations
int64_t rust_fib(int64_t n);
int64_t rust_sum_squares(int64_t n);

#endif // MIMI_FFI_H

// Exported Mimi functions (extern "C")
int64_t mimi_fib(int64_t n);
int64_t mimi_sum_squares(int64_t n);
