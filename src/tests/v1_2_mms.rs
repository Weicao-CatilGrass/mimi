use super::*;

#[test]
fn mms_block_basic() {
    let src = r#"
func main() -> i32 {
    mms {
        "func pay requires: balance >= amount"
    }
    42
}
"#;
    // mms block should parse and be ignored at runtime
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn mms_block_with_code() {
    let src = r#"
func pay(amount: i32) {
    mms {
        func Pay(amount):
            desc "Process payment"
            requires: amount > 0
    }
    println(amount);
}

func main() -> i32 {
    pay(100);
    42
}
"#;
    // mms block inside a function should work
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn mms_block_multiple() {
    let src = r#"
func main() -> i32 {
    mms {
        "Step 1: check balance"
    }
    mms {
        "Step 2: charge payment"
    }
    42
}
"#;
    // Multiple mms blocks should work
    assert_eq!(run_source(src), interp::Value::Int(42));
}
