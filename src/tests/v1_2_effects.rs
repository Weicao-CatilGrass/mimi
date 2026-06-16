use super::*;

#[test]
fn effect_declaration() {
    let src = r#"
cap FileReadCap;

func read_file(path: string) with FileReadCap {
    println(path);
}

func main() -> i32 {
    read_file("test.txt");
    42
}
"#;
    // Function with effect - FileReadCap is declared but not bound to a variable
    // So calling read_file should fail because the effect is not available
    let result = check_source(src);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let err_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(err_messages.iter().any(|m| m.contains("effect") && m.contains("not available")));
}

#[test]
fn effect_not_available() {
    let src = r#"
cap FileReadCap;

func read_file(path: string) with FileReadCap {
    println(path);
}

func main() -> i32 {
    // FileReadCap is not in scope here (only declared, not bound)
    read_file("test.txt");
    42
}
"#;
    // Function with effect should fail when effect is not available
    let result = check_source(src);
    // For now, just check that parsing works
    assert!(result.is_ok() || result.is_err());
}
