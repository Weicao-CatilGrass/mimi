//! Safe arithmetic operations with overflow detection.
//!
//! All integer arithmetic in the interpreter **must** go through this module
//! rather than using raw Rust `+`, `-`, `*`, etc. on `i64`.  This ensures
//! that two's-complement wrapping (which is UB-free in Rust but is almost
//! never the intended semantics for Mimi programs) is rejected at runtime.
//!
//! Every function returns `None` when the result would overflow (or, for
//! division / modulo, when `b == 0`).  Callers must inspect the `Option`
//! and produce an appropriate `InterpError`.

/// Checked addition - returns None on overflow
pub fn checked_add(a: i64, b: i64) -> Option<i64> {
    a.checked_add(b)
}

/// Checked subtraction - returns None on overflow
pub fn checked_sub(a: i64, b: i64) -> Option<i64> {
    a.checked_sub(b)
}

/// Checked multiplication - returns None on overflow
pub fn checked_mul(a: i64, b: i64) -> Option<i64> {
    a.checked_mul(b)
}

/// Checked division - returns None on division by zero
pub fn checked_div(a: i64, b: i64) -> Option<i64> {
    a.checked_div(b)
}

/// Checked modulo - returns None on division by zero
pub fn checked_rem(a: i64, b: i64) -> Option<i64> {
    a.checked_rem(b)
}

/// Checked negation - returns None on overflow (i64::MIN)
pub fn checked_neg(a: i64) -> Option<i64> {
    a.checked_neg()
}

/// Checked power - returns None on overflow
pub fn checked_pow(base: i64, exp: u32) -> Option<i64> {
    base.checked_pow(exp)
}

/// Checked left shift - returns None if shift amount >= 64
pub fn checked_shl(a: i64, b: u32) -> Option<i64> {
    a.checked_shl(b)
}

/// Checked right shift - returns None if shift amount >= 64
pub fn checked_shr(a: i64, b: u32) -> Option<i64> {
    a.checked_shr(b)
}

/// Checked absolute value - returns None on overflow (i64::MIN)
pub fn checked_abs(a: i64) -> Option<i64> {
    a.checked_abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checked_add_success() {
        assert_eq!(checked_add(1, 2), Some(3));
        assert_eq!(checked_add(i64::MAX, 0), Some(i64::MAX));
    }

    #[test]
    fn test_checked_add_overflow() {
        assert_eq!(checked_add(i64::MAX, 1), None);
        assert_eq!(checked_add(i64::MAX, i64::MAX), None);
    }

    #[test]
    fn test_checked_sub_success() {
        assert_eq!(checked_sub(5, 3), Some(2));
        assert_eq!(checked_sub(i64::MIN, 0), Some(i64::MIN));
    }

    #[test]
    fn test_checked_sub_overflow() {
        assert_eq!(checked_sub(i64::MIN, 1), None);
        assert_eq!(checked_sub(i64::MIN, i64::MAX), None);
    }

    #[test]
    fn test_checked_mul_success() {
        assert_eq!(checked_mul(3, 4), Some(12));
        assert_eq!(checked_mul(i64::MAX, 1), Some(i64::MAX));
    }

    #[test]
    fn test_checked_mul_overflow() {
        assert_eq!(checked_mul(i64::MAX, 2), None);
        assert_eq!(checked_mul(i64::MAX, i64::MAX), None);
    }

    #[test]
    fn test_checked_div_success() {
        assert_eq!(checked_div(10, 2), Some(5));
        assert_eq!(checked_div(-10, 2), Some(-5));
    }

    #[test]
    fn test_checked_div_by_zero() {
        assert_eq!(checked_div(10, 0), None);
    }

    #[test]
    fn test_checked_rem_success() {
        assert_eq!(checked_rem(10, 3), Some(1));
        assert_eq!(checked_rem(10, 2), Some(0));
    }

    #[test]
    fn test_checked_rem_by_zero() {
        assert_eq!(checked_rem(10, 0), None);
    }

    #[test]
    fn test_checked_neg_success() {
        assert_eq!(checked_neg(5), Some(-5));
        assert_eq!(checked_neg(-5), Some(5));
        assert_eq!(checked_neg(0), Some(0));
    }

    #[test]
    fn test_checked_neg_overflow() {
        assert_eq!(checked_neg(i64::MIN), None);
    }

    #[test]
    fn test_checked_pow_success() {
        assert_eq!(checked_pow(2, 10), Some(1024));
        assert_eq!(checked_pow(0, 0), Some(1));
    }

    #[test]
    fn test_checked_pow_overflow() {
        assert_eq!(checked_pow(i64::MAX, 2), None);
    }

    #[test]
    fn test_checked_shl_success() {
        assert_eq!(checked_shl(1, 10), Some(1024));
        assert_eq!(checked_shl(1, 0), Some(1));
    }

    #[test]
    fn test_checked_shl_overflow() {
        // Shift by 63 is valid for i64 (wraps into sign bit)
        assert!(checked_shl(1, 63).is_some());
        // Shift by 64 or more returns None (exceeds bit width)
        assert_eq!(checked_shl(1, 64), None);
        assert_eq!(checked_shl(1, 128), None);
    }

    #[test]
    fn test_checked_shr_success() {
        assert_eq!(checked_shr(1024, 10), Some(1));
        assert_eq!(checked_shr(1, 0), Some(1));
    }

    #[test]
    fn test_checked_shr_overflow() {
        assert_eq!(checked_shr(1, 64), None);
    }

    #[test]
    fn test_checked_abs_success() {
        assert_eq!(checked_abs(5), Some(5));
        assert_eq!(checked_abs(-5), Some(5));
        assert_eq!(checked_abs(0), Some(0));
    }

    #[test]
    fn test_checked_abs_overflow() {
        assert_eq!(checked_abs(i64::MIN), None);
    }
}
