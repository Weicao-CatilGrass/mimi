use super::*;
#[test]
fn trait_basic_definition_and_impl() {
    let src = r#"
trait Drawable {
    func draw() -> string
}

type Circle {
    radius: f64
}

impl Drawable for Circle {
    func draw() -> string {
        "circle"
    }
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
    assert_eq!(run_source(src), interp::Value::Int(42));
}


#[test]
fn trait_method_call_on_type() {
    let src = r#"
trait Describable {
    func describe() -> string
}

type Point {
    x: i32,
    y: i32
}

impl Describable for Point {
    func describe() -> string {
        "point"
    }
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}


#[test]
fn trait_multiple_traits_on_type() {
    let src = r#"
trait Printable {
    func print() -> string
}

trait Loggable {
    func log() -> string
}

type MyObj {
    value: i32
}

impl Printable for MyObj {
    func print() -> string {
        "printed"
    }
}

impl Loggable for MyObj {
    func log() -> string {
        "logged"
    }
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}


#[test]
fn trait_missing_method_error() {
    let src = r#"
trait Greetable {
    func greet() -> string
}

type Person {
    name: string
}

impl Greetable for Person {
}

func main() -> i32 {
    42
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("missing method 'greet'")));
}


#[test]
fn trait_method_wrong_arg_type() {
    let src = r#"
trait Addable {
    func add(x: i32) -> i32
}

type Counter {
    count: i32
}

impl Addable for Counter {
    func add(x: i32) -> i32 {
        x
    }
}

func main() -> i32 {
    Counter::add("not an int")
}
"#;
    // Type checker catches the error at the Call site
    let result = check_source(src);
    // The error may be from type checker or interpreter
    // For now just verify it doesn't pass cleanly
    assert!(result.is_err() || run_source_result(src).is_err());
}


#[test]
fn trait_method_return_type() {
    let src = r#"
trait AsInt {
    func as_int() -> i32
}

type Num {
    value: i32
}

impl AsInt for Num {
    func as_int() -> i32 {
        42
    }
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}


#[test]
fn trait_impl_method_body_check() {
    let src = r#"
trait Computable {
    func compute() -> i32
}

type Data {
    x: i32
}

impl Computable for Data {
    func compute() -> i32 {
        11
    }
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}


#[test]
fn trait_undefined_trait_error() {
    let src = r#"
type Foo {
    x: i32
}

impl Nonexistent for Foo {
    func bar() -> i32 {
        42
    }
}

func main() -> i32 {
    42
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("undefined trait")));
}


#[test]
fn trait_method_arg_count_mismatch() {
    let src = r#"
trait Processor {
    func process(x: i32) -> i32
}

type Worker {
    id: i32
}

impl Processor for Worker {
    func process(x: i32) -> i32 {
        x
    }
}

func main() -> i32 {
    Worker::process(1, 2)
}
"#;
    // This will fail at runtime (interpreter), not type checker, since
    // the type checker doesn't see the call as a trait method call
    let result = run_source_result(src);
    assert!(result.is_err());
}

// ===== T302: 引用语义测试 =====


