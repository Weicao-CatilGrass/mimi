use super::*;

#[test]
fn typecheck_double_mut_borrow_error() {
    let src = r#"
func main() -> i32 {
    let mut x = 42;
    let r1 = &mut x;
    let r2 = &mut x;
    1
}
"#;
    let file = parse(src);
    let result = core::check(&file);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let has_borrow_error = errors.iter().any(|e| e.message.contains("already mutably borrowed"));
    assert!(has_borrow_error, "Expected mutable borrow error, got: {:?}", errors);
}

#[test]
fn typecheck_imm_mut_borrow_error() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r1 = &x;
    let r2 = &mut x;
    1
}
"#;
    let file = parse(src);
    let result = core::check(&file);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let has_borrow_error = errors.iter().any(|e| e.message.contains("already immutably borrowed"));
    assert!(has_borrow_error, "Expected immutable borrow error, got: {:?}", errors);
}

#[test]
fn typecheck_double_imm_borrow_ok() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r1 = &x;
    let r2 = &x;
    1
}
"#;
    let file = parse(src);
    let result = core::check(&file);
    assert!(result.is_ok(), "Multiple immutable borrows should be allowed");
}

#[test]
fn typecheck_borrow_scope_isolation() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    {
        let r = &mut x;
    }
    let r2 = &x;
    1
}
"#;
    let file = parse(src);
    let result = core::check(&file);
    assert!(result.is_ok(), "Borrows should be isolated to their scope");
}
