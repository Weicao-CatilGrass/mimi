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

#[test]
fn debug_commitment_tokens() {
    use crate::ast::Commitment;
    let src = "func$ add(a: i32, b: i32) -> i32 {\n    a + b\n}";
    let tokens = lexer::Lexer::new(src).tokenize().unwrap();
    for (i, t) in tokens.iter().enumerate() {
        if t.commitment != Commitment::None {
            eprintln!("TOKEN[{}] {:?} commitment={:?}", i, t.kind, t.commitment);
        }
    }
    let has_commitment = tokens.iter().any(|t| t.commitment != Commitment::None);
    eprintln!("Has commitment: {}", has_commitment);
}
