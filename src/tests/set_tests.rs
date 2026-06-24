use super::*;

// ─── Set literal + operations (v0.22.3) ───────────────────────

#[test]
fn set_literal_basic() {
    let v = run_source(r#"func main() -> i32 { let s = {1, 2, 3}; s.size() }"#);
    assert_eq!(v, interp::Value::Int(3));
}

#[test]
fn set_literal_dedup() {
    let v = run_source(r#"func main() -> i32 { let s = {1, 1, 2, 2, 3}; s.size() }"#);
    assert_eq!(v, interp::Value::Int(3));
}

#[test]
fn set_literal_single_is_block() {
    // {42} is a block expression, not a set literal
    let v = run_source(r#"func main() -> i32 { {42} }"#);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn set_contains_true() {
    let v = run_source(r#"func main() -> bool { let s = {10, 20, 30}; s.contains(20) }"#);
    assert_eq!(v, interp::Value::Bool(true));
}

#[test]
fn set_contains_false() {
    let v = run_source(r#"func main() -> bool { let s = {10, 20, 30}; s.contains(99) }"#);
    assert_eq!(v, interp::Value::Bool(false));
}

#[test]
fn set_size_empty() {
    let v = run_source(r#"func main() -> i32 { let s = {1, 2}; let s2 = s.remove(1).remove(2); s2.size() }"#);
    assert_eq!(v, interp::Value::Int(0));
}

#[test]
fn set_insert_returns_new_set() {
    let v = run_source(r#"func main() -> i32 { let s = {1, 2}; s.insert(3).size() }"#);
    assert_eq!(v, interp::Value::Int(3));
}

#[test]
fn set_remove_returns_new_set() {
    let v = run_source(r#"func main() -> i32 { let s = {1, 2, 3}; s.remove(2).size() }"#);
    assert_eq!(v, interp::Value::Int(2));
}

#[test]
fn set_insert_duplicate_no_change() {
    let v = run_source(r#"func main() -> i32 { let s = {1, 2}; s.insert(2).size() }"#);
    assert_eq!(v, interp::Value::Int(2));
}

#[test]
fn set_is_empty_false() {
    let v = run_source(r#"func main() -> bool { let s = {1, 2}; s.is_empty() }"#);
    assert_eq!(v, interp::Value::Bool(false));
}

#[test]
fn set_is_empty_true() {
    let v = run_source(r#"func main() -> bool { let s = {1, 2}; s.remove(1).remove(2).is_empty() }"#);
    assert_eq!(v, interp::Value::Bool(true));
}

#[test]
fn set_to_list() {
    let v = run_source(r#"func main() -> i32 {
        let s = {1, 2, 3};
        let lst = s.to_list();
        lst.len()
    }"#);
    assert_eq!(v, interp::Value::Int(3));
}

#[test]
fn set_string_elements() {
    let v = run_source(r#"func main() -> i32 {
        let s = {"a", "b", "c"};
        s.size()
    }"#);
    assert_eq!(v, interp::Value::Int(3));
}

#[test]
fn set_contains_string() {
    let v = run_source(r#"func main() -> bool {
        let s = {"hello", "world"};
        s.contains("hello")
    }"#);
    assert_eq!(v, interp::Value::Bool(true));
}

#[test]
fn set_chain_operations() {
    let v = run_source(r#"func main() -> i32 {
        let s = {1, 2, 3};
        s.insert(4).remove(2).size()
    }"#);
    assert_eq!(v, interp::Value::Int(3));
}
