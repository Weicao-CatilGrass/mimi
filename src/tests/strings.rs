use super::*;

#[test]
fn string_concat() {
    let src = r#"
func main() -> string {
    let s = "hello" + " " + "world";
    s
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::String("hello world".to_string()));
}

#[test]
fn string_concat_empty() {
    let src = r#"
func main() -> string {
    let s = "" + "abc" + "";
    s
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::String("abc".to_string()));
}

#[test]
fn fstring_basic() {
    let src = r#"
func main() -> string {
    let name = "World";
    f"Hello, {name}!"
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::String("Hello, World!".to_string()));
}

#[test]
fn fstring_multiple_interpolations() {
    let src = r#"
func main() -> string {
    let a = 1;
    let b = 2;
    f"{a} + {b} = {a + b}"
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::String("1 + 2 = 3".to_string()));
}

#[test]
fn fstring_no_interpolation() {
    let src = r#"
func main() -> string {
    f"just text"
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::String("just text".to_string()));
}

#[test]
fn fstring_expression_interpolation() {
    let src = r#"
func main() -> string {
    let x = 10;
    f"double is {x * 2}"
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::String("double is 20".to_string()));
}

#[test]
fn fstring_with_function_call() {
    let src = r#"
func greet(name: string) -> string {
    f"Hi, {name}!"
}

func main() -> string {
    greet("Alice")
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::String("Hi, Alice!".to_string()));
}
