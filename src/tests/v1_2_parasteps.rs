use super::*;

#[test]
fn parasteps_local_shared_not_allowed() {
    let src = r#"
func main() -> i32 {
    local_shared x = 42;
    parasteps {
        println(x);
    }
    42
}
"#;
    // local_shared cannot be captured in parasteps
    let result = check_source(src);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let err_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(err_messages.iter().any(|m| m.contains("local_shared")));
}

#[test]
fn parasteps_shared_allowed() {
    let src = r#"
func main() -> i32 {
    shared x = 42;
    parasteps {
        println(x);
    }
    42
}
"#;
    // shared can be captured in parasteps
    let result = check_source(src);
    assert!(result.is_ok());
}
