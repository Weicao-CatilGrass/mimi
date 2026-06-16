use super::*;
use crate::ast::Item;

#[test]
fn parse_pub_func() {
    let src = r#"
pub func helper() -> i32 { 42 }

func main() -> i32 {
    helper()
}
"#;
    let file = parse(src);
    if let Item::Func(f) = &file.items[0] {
        assert!(f.pub_, "func should be marked as pub");
    } else {
        panic!("expected Func item");
    }
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn parse_pub_type() {
    let src = r#"
pub type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    1
}
"#;
    let file = parse(src);
    if let Item::Type(t) = &file.items[0] {
        assert!(t.pub_, "type should be marked as pub");
    } else {
        panic!("expected Type item");
    }
}

#[test]
fn parse_non_pub_func() {
    let src = r#"
func helper() -> i32 { 42 }

func main() -> i32 {
    helper()
}
"#;
    let file = parse(src);
    if let Item::Func(f) = &file.items[0] {
        assert!(!f.pub_, "func without pub should not be marked as pub");
    } else {
        panic!("expected Func item");
    }
}
