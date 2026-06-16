use super::*;

#[test]
fn interp_tuple_destructuring() {
    let src = r#"
func main() -> i32 {
    let (a, b, c) = (1, 2, 3);
    a + b + c
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(6));
}

#[test]
fn interp_unit_in_tuple() {
    let src = r#"
func main() -> i32 {
    let t = ((), 42);
    let (_, x) = t;
    x
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}
