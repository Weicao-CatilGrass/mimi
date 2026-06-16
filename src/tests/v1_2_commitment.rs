use super::*;

#[test]
fn commitment_locked_parsing() {
    let src = r#"
func add(a: i32, b: i32) -> i32$ {
    a + b
}

func main() -> i32 {
    add(1, 2)
}
"#;
    // $ locked function should parse and run
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn commitment_question_parsing() {
    let src = r#"
func add(a: i32, b: i32) -> i32? {
    a + b
}

func main() -> i32 {
    add(1, 2)
}
"#;
    // ? uncertain function should parse and run
    assert_eq!(run_source(src), interp::Value::Int(3));
}
