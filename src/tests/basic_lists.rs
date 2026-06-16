use super::*;

#[test]
fn interp_list_access() {
    let src = r#"
func main() -> i32 {
    let xs = [1, 2, 3, 4, 5];
    xs[0] + xs[4]
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(6));
}
