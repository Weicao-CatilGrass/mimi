#include <stdint.h>
#include <string.h>
#include <stdbool.h>

static int64_t do_reverse(int64_t n) {
    int64_t rev = 0;
    int64_t tmp = n < 0 ? -n : n;
    while (tmp > 0) {
        rev = rev * 10 + tmp % 10;
        tmp /= 10;
    }
    return n < 0 ? -rev : rev;
}

static int64_t do_count_digits(int64_t n) {
    int64_t tmp = n < 0 ? -n : n;
    if (tmp == 0) return 1;
    int64_t count = 0;
    while (tmp > 0) { count++; tmp /= 10; }
    return count;
}

static bool do_is_palindrome(int64_t n) {
    if (n < 0) return false;
    return n == do_reverse(n);
}

static int64_t do_gcd(int64_t a, int64_t b) {
    if (a < 0) a = -a;
    if (b < 0) b = -b;
    while (b != 0) { int64_t t = b; b = a % b; a = t; }
    return a;
}

// Mimi codegen calls __mimi_extern_<name> for extern "C" block functions
int64_t __mimi_extern_c_reverse(int64_t n) { return do_reverse(n); }
int64_t __mimi_extern_c_count_digits(int64_t n) { return do_count_digits(n); }
bool    __mimi_extern_c_is_palindrome(int64_t n) { return do_is_palindrome(n); }
int64_t __mimi_extern_c_gcd(int64_t a, int64_t b) { return do_gcd(a, b); }

// Also export plain names for C header / pybind consumers
int64_t c_reverse(int64_t n) { return __mimi_extern_c_reverse(n); }
int64_t c_count_digits(int64_t n) { return __mimi_extern_c_count_digits(n); }
bool    c_is_palindrome(int64_t n) { return __mimi_extern_c_is_palindrome(n); }
int64_t c_gcd(int64_t a, int64_t b) { return __mimi_extern_c_gcd(a, b); }
