use super::*;

#[test]
fn interp_if_else() {
    let src = r#"
func main() -> i32 {
    let x = 5;
    if x > 3 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(1));
}

#[test]
fn interp_while() {
    let src = r#"
func main() -> i32 {
    let mut i = 0;
    let mut sum = 0;
    while i < 5 {
        sum = sum + i;
        i = i + 1;
    }
    return sum;
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn interp_for_range() {
    let src = r#"
func main() -> i32 {
    let mut sum = 0;
    for i in range(0, 5) {
        sum = sum + i;
    }
    return sum;
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn interp_fib() {
    let src = r#"
func fib(n: i32) -> i32 {
    if n <= 1 {
        return n;
    } else {
        return fib(n - 1) + fib(n - 2);
    }
}

func main() -> i32 {
    return fib(10);
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(55));
}

#[test]
fn typecheck_if_condition_bool() {
    let src = r#"
func main() {
    if 42 {
        println("bad");
    }
}
"#;
    let errs = check_source(src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("if condition must be bool")));
}

#[test]
fn interp_match_enum() {
    let src = r#"
type Shape {
    Circle(f64)
    Rectangle(f64, f64)
}

func area(s: Shape) -> f64 {
    match s {
        Circle(r) => 3.14159 * r * r,
        Rectangle(w, h) => w * h,
    }
}

func main() -> f64 {
    area(Circle(2.0)) + area(Rectangle(3.0, 4.0))
}
"#;
    let v = run_source(src);
    assert!(matches!(v, interp::Value::Float(_)));
}

#[test]
fn typecheck_match_exhaustive() {
    let src = r#"
type Opt { Some(i32) None }
func main() -> i32 {
    let x = Some(42);
    match x {
        Some(v) => v,
        None => 0,
    }
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn typecheck_match_non_exhaustive() {
    let src = r#"
type Color {
    Red
    Green
    Blue
}

func main() -> i32 {
    let c = Red;
    match c {
        Red => 1,
        Green => 2,
    }
}
"#;
    let errs = check_source(src).unwrap_err();
    assert!(!errs.is_empty());
}

#[test]
fn interp_match_with_guard() {
    let src = r#"
type Opt {
    Some(i32)
    None
}

func main() -> i32 {
    let x = Some(5);
    match x {
        Some(n) if n > 3 => 1,
        Some(n) if n <= 3 => 2,
        None => 0,
    }
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(1));
}

#[test]
fn interp_match_nested_variants() {
    let src = r#"
type Tree {
    Leaf(i32)
    Node(Tree, Tree)
}

func sum(t: Tree) -> i32 {
    match t {
        Leaf(n) => n,
        Node(l, r) => sum(l) + sum(r),
    }
}

func main() -> i32 {
    let t = Node(Leaf(1), Node(Leaf(2), Leaf(3)));
    sum(t)
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(6));
}

#[test]
fn interp_match_tuple_pattern() {
    let src = r#"
type Pair {
    Pair(i32, i32)
}

func main() -> i32 {
    let p = Pair(10, 20);
    match p {
        Pair(a, b) => a + b,
    }
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(30));
}

#[test]
fn interp_else_if_chain() {
    let src = r#"
func classify(n: i32) -> i32 {
    if n < 0 {
        -1
    } else if n == 0 {
        0
    } else {
        1
    }
}

func main() -> i32 {
    classify(-5) + classify(0) + classify(10)
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(0));
}

#[test]
fn interp_else_if_multiple() {
    let src = r#"
func grade(score: i32) -> i32 {
    if score >= 90 {
        4
    } else if score >= 80 {
        3
    } else if score >= 70 {
        2
    } else if score >= 60 {
        1
    } else {
        0
    }
}

func main() -> i32 {
    grade(95) + grade(85) + grade(75) + grade(65) + grade(50)
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(10));
}

#[test]
fn interp_nested_match() {
    let src = r#"
type Opt {
    Some(i32)
    None
}

func unwrap_or(o: Opt, default: i32) -> i32 {
    match o {
        Some(v) => v,
        None => default,
    }
}

func main() -> i32 {
    unwrap_or(Some(42), 0) + unwrap_or(None, 10)
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(52));
}
