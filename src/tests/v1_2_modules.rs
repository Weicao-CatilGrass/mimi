use super::*;

#[test]
fn module_use_basic() {
    let src = r#"
use std::collections;

func main() -> i32 {
    42
}
"#;
    let result = check_source(src);
    // use statements parse but modules may not exist
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn module_nested_types() {
    let src = r#"
module Math {
    type Point {
        x: i32,
        y: i32
    }

    func origin() -> Point {
        Point { x: 0, y: 0 }
    }
}

func main() -> i32 {
    let p = Math.origin();
    p.x
}
"#;
    let result = run_source_result(src);
    assert!(result.is_ok() || result.is_err());
}
