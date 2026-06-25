use super::*;
#[test]
fn builtin_map() {
    let src = r#"
func main() -> i32 {
    let nums = [1, 2, 3];
    let doubled = map(nums, fn(x: i32) -> i32 { x * 2 });
    doubled[0] + doubled[1] + doubled[2]
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(12));
}

#[test]
fn builtin_filter() {
    let src = r#"
func main() -> i32 {
    let nums = [1, 2, 3, 4, 5];
    let evens = filter(nums, fn(x: i32) -> bool { x % 2 == 0 });
    len(evens)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(2));
}

#[test]
fn builtin_reduce() {
    let src = r#"
func main() -> i32 {
    let nums = [1, 2, 3, 4];
    reduce(nums, fn(acc: i32, x: i32) -> i32 { acc + x }, 0)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn builtin_sort() {
    let src = r#"
func main() -> i32 {
    let nums = [3, 1, 4, 1, 5];
    let sorted = sort(nums);
    sorted[0] + sorted[1] + sorted[2]
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(5));
}

#[test]
fn builtin_reverse() {
    let src = r#"
func main() -> i32 {
    let nums = [1, 2, 3];
    let rev = reverse(nums);
    rev[0]
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn builtin_enumerate() {
    let src = r#"
func main() -> i32 {
    let items = ["a", "b", "c"];
    let enums = enumerate(items);
    len(enums)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn builtin_zip() {
    let src = r#"
func main() -> i32 {
    let a = [1, 2, 3];
    let b = [4, 5, 6];
    let zipped = zip(a, b);
    len(zipped)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn builtin_flatten() {
    let src = r#"
func main() -> i32 {
    let nested = [[1, 2], [3, 4], [5]];
    let flat = flatten(nested);
    len(flat)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(5));
}

#[test]
fn builtin_sum() {
    let src = r#"
func main() -> i32 {
    let nums = [1, 2, 3, 4, 5];
    sum(nums)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(15));
}

#[test]
fn builtin_assert_eq_pass() {
    let src = r#"
func main() -> i32 {
    assert_eq(1 + 1, 2);
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn builtin_assert_eq_fail() {
    let src = r#"
func main() -> i32 {
    assert_eq(1 + 1, 3);
    42
}
"#;
    let err = run_source_result(src);
    assert!(err.is_err());
}

#[test]
fn builtin_assert_ne_pass() {
    let src = r#"
func main() -> i32 {
    assert_ne(1, 2);
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}
