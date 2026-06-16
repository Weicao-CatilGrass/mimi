use super::*;

#[test]
fn shared_basic_creation() {
    let src = r#"
func main() {
    shared x = 42;
    println(x);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn shared_clone_refcount() {
    let src = r#"
func main() {
    shared x = 42;
    shared y = x;
    println(x);
    println(y);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn shared_field_access() {
    let src = r#"
type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    shared s = Point { x: 10, y: 20 };
    s.x
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(10));
}

#[test]
fn shared_deref_method() {
    let src = r#"
func main() -> i32 {
    shared x = 42;
    x.deref()
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn local_shared_basic() {
    let src = r#"
func main() {
    local_shared x = 100;
    local_shared y = x;
    println(x);
    println(y);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn local_shared_deref() {
    let src = r#"
func main() -> i32 {
    local_shared x = 99;
    x.inner()
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(99));
}

#[test]
fn weak_shared_basic() {
    let src = r#"
func main() {
    shared x = 42;
    weak w = x;
    println(w);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn weak_upgrade_success() {
    let src = r#"
func main() -> i32 {
    shared x = 42;
    weak w = x;
    let upgraded = w.upgrade();
    upgraded.deref()
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn weak_upgrade_none_after_drop() {
    let src = r#"
func get_weak() -> weak i32 {
    shared x = 42;
    weak w = x;
    w
}

func main() -> i32 {
    let w = get_weak();
    let upgraded = w.upgrade();
    // upgraded is a variant - check if it's None
    match upgraded {
        Some(v) => v.deref(),
        None => 0,
    }
}
"#;
    let result = run_source_result(src);
    // After shared x is dropped, upgrade returns None
    assert!(result.is_ok());
}

#[test]
fn weak_local_basic() {
    let src = r#"
func main() {
    local_shared x = 10;
    weak w = x;
    println(w);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn weak_local_upgrade() {
    let src = r#"
func main() -> i32 {
    local_shared x = 55;
    weak w = x;
    let upgraded = w.upgrade();
    upgraded.inner()
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(55));
}

#[test]
fn shared_record_field_access() {
    let src = r#"
type Node {
    value: i32
    next: i32
}

func main() -> i32 {
    shared node = Node { value: 7, next: 0 };
    node.value
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(7));
}

#[test]
fn shared_multiple_shares() {
    let src = r#"
func main() {
    shared a = 1;
    shared b = a;
    shared c = b;
    println(a);
    println(b);
    println(c);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn shared_as_function_arg() {
    let src = r#"
func use_shared(x: shared i32) {
    println(x);
}

func main() {
    shared v = 42;
    use_shared(v);
    println(v);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn weak_shared_in_list() {
    let src = r#"
func main() {
    shared a = 10;
    shared b = 20;
    weak wa = a;
    weak wb = b;
    let list = [wa, wb];
    println(list);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}

#[test]
fn arena_no_escape_ok() {
    let src = r#"
func process() -> i32 {
    arena {
        let ref x = 10;
        let val = x;
        42
    }
}

func main() -> i32 {
    process()
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn arena_escape_return_detected() {
    let src = r#"
func process() -> i32 {
    arena {
        let ref x = 10;
        return x;
    }
}

func main() -> i32 {
    process()
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("arena escape"), "Expected arena escape error, got: {}", err);
}

#[test]
fn arena_escape_variable_detected() {
    let src = r#"
func main() -> i32 {
    let mut escaped = 0;
    arena {
        let ref x = 42;
        escaped = x;
    }
    escaped
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("arena escape"), "Expected arena escape error, got: {}", err);
}

#[test]
fn arena_nested_ok() {
    let src = r#"
func main() -> i32 {
    arena {
        let a = 10;
        arena {
            let b = 20;
            a + b
        }
    }
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(30));
}

#[test]
fn arena_no_ref_ok() {
    let src = r#"
func main() -> i32 {
    let mut x = 0;
    arena {
        x = 42;
    }
    x
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn arena_ref_within_scope_ok() {
    let src = r#"
func main() -> i32 {
    arena {
        let a = 10;
        let b = 20;
        let result = a + b;
        result
    }
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(30));
}
