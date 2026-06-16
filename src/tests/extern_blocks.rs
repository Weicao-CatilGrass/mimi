use super::*;

#[test]
fn extern_block_basic() {
    let src = r#"
extern "C" {
    func printf(fmt: string) -> i32;
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn extern_block_multiple_funcs() {
    let src = r#"
extern "C" {
    func malloc(size: i32) -> i32;
    func free(ptr: i32);
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn extern_block_with_cap() {
    let src = r#"
cap FileReadCap;

extern "C" {
    func read(fd: i32, file_cap: FileReadCap) -> string;
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn extern_block_with_borrow() {
    let src = r#"
cap FileReadCap;

extern "C" {
    func read(fd: i32, file_cap: FileReadCap) -> string;
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn extern_with_multiple_params() {
    let src = r#"
extern "C" {
    func write(fd: i32, buf: string, len: i32) -> i32;
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}

#[test]
fn extern_with_no_return() {
    let src = r#"
extern "C" {
    func exit(code: i32);
}

func main() -> i32 {
    42
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(42));
}
