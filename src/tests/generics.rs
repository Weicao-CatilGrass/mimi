use super::*;

#[test]
fn trait_definition() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn trait_with_impl() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

type MyType {
    value: i32
}

impl Display for MyType {
    func to_string() -> string {
        return "MyType";
    }
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn trait_multiple_methods() {
    let src = r#"
trait Printable {
    func to_string() -> string;
    func print();
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn trait_with_params() {
    let src = r#"
trait Addable {
    func add(x: i32) -> i32;
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn where_single_constraint() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

type MyType {
    value: i32
}

func print(x: MyType) where MyType: Display {
    println(x);
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn where_multiple_constraints() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

trait Clone {
    func clone() -> Self;
}

type MyType {
    value: i32
}

func process(x: MyType) where MyType: Display + Clone {
    println(x);
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn where_with_return_type() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

type MyType {
    value: i32
}

func format(x: MyType) -> string where MyType: Display {
    x.to_string()
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn trait_with_multiple_methods_impl() {
    let src = r#"
trait Printable {
    func to_string() -> string;
    func print();
}

type MyItem {
    value: i32
}

impl Printable for MyItem {
    func to_string() -> string {
        return "MyItem";
    }
    func print() {
        println("MyItem");
    }
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn where_with_multiple_bounds() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

trait Clone {
    func clone() -> Self;
}

type MyType {
    value: i32
}

func process(x: MyType) -> string where MyType: Display + Clone {
    x.to_string()
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}
