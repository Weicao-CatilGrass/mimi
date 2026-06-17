use crate::tests::*;
use proptest::prelude::*;

proptest! {
    #[test]
    fn eval_int_literal(n in -1000i64..1000i64) {
        let src = format!("func main() -> i64 {{ {} }}", n);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, n);
        }
    }

    #[test]
    fn eval_int_addition(a in -100i64..100, b in -100i64..100) {
        let src = format!("func main() -> i64 {{ {} + {} }}", a, b);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, a.wrapping_add(b));
        }
    }

    #[test]
    fn eval_int_multiply(a in -50i64..50, b in -50i64..50) {
        let src = format!("func main() -> i64 {{ {} * {} }}", a, b);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, a.wrapping_mul(b));
        }
    }

    #[test]
    fn eval_int_negate(n in -1000i64..1000) {
        let src = format!("func main() -> i64 {{ -{} }}", n);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, n.wrapping_neg());
        }
    }

    #[test]
    fn eval_bool_not(b in any::<bool>()) {
        let src = format!("func main() -> bool {{ !{} }}", b);
        if let crate::interp::Value::Bool(result) = run_source(&src) {
            prop_assert_eq!(result, !b);
        }
    }

    #[test]
    fn eval_string_length(s in "[a-z]{0,50}") {
        let src = format!("func main() -> i64 {{ len(\"{}\") }}", s);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, s.len() as i64);
        }
    }

    #[test]
    fn eval_range_for_sum(n in 1i64..20) {
        let src = format!(r#"
func main() -> i64 {{
    let mut sum = 0;
    for i in 0..{} {{
        sum = sum + i;
    }}
    sum
}}"#, n);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            let expected = n * (n - 1) / 2;
            prop_assert_eq!(result, expected, "n={}", n);
        }
    }

    #[test]
    fn eval_if_else(a in -100i64..100, b in -100i64..100) {
        let src = format!("func main() -> i64 {{ if {} > 0 {{ {} }} else {{ {} }} }}", a, a, b);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            let expected = if a > 0 { a } else { b };
            prop_assert_eq!(result, expected);
        }
    }

    #[test]
    fn eval_while_loop(n in 0i64..50) {
        let src = format!(r#"
func main() -> i64 {{
    let mut i = 0;
    let mut sum = 0;
    while i < {} {{
        sum = sum + i;
        i = i + 1;
    }}
    sum
}}"#, n);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            let expected = n * (n - 1) / 2;
            prop_assert_eq!(result, expected, "n={}", n);
        }
    }

    #[test]
    fn eval_func_composition(a in -50i64..50) {
        let src = format!(r#"
func double(x: i64) -> i64 {{
    x * 2
}}
func add_one(x: i64) -> i64 {{
    x + 1
}}
func main() -> i64 {{
    double(add_one({}))
}}"#, a);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, (a + 1) * 2);
        }
    }

    #[test]
    fn eval_list_len(n in 0usize..20) {
        let items: Vec<String> = (0..n).map(|i| format!("{}", i)).collect();
        let list_str = items.join(", ");
        let src = format!("func main() -> i64 {{ len([{}]) }}", list_str);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, n as i64);
        }
    }

    #[test]
    fn eval_type_name_int(n in 1i64..100) {
        let src = format!("func main() -> string {{ type_name({}) }}", n);
        if let crate::interp::Value::String(result) = run_source(&src) {
            prop_assert!(result == "i64" || result == "i32", "unexpected type_name: {}", result);
        }
    }

    #[test]
    fn eval_pow(base in 0i64..10, exp in 0u32..6) {
        let src = format!("func main() -> i64 {{ pow({}, {}) }}", base, exp);
        if let crate::interp::Value::Int(result) = run_source(&src) {
            prop_assert_eq!(result, base.pow(exp));
        }
    }
}
