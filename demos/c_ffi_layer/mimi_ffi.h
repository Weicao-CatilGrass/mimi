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
int64_t c_reverse(int64_t n);
int64_t c_count_digits(int64_t n);
int64_t c_is_palindrome(int64_t n);
int64_t c_gcd(int64_t a, int64_t b);

#endif // MIMI_FFI_H

// Exported Mimi functions (extern "C")
int64_t reverse(int64_t n);
int64_t count_digits(int64_t n);
bool is_palindrome(int64_t n);
int64_t gcd(int64_t a, int64_t b);
