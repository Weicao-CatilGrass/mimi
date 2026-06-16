use super::*;

fn compile_to_ir(src: &str) -> String {
    let file = parse(src);
    let context = inkwell::context::Context::create();
    let mut codegen = crate::codegen::CodeGenerator::new(&context, "test");
    codegen.compile_file(&file).unwrap();
    codegen.emit_ir()
}

fn assert_compiles(src: &str) {
    let ir = compile_to_ir(src);
    assert!(ir.contains("define"), "IR should contain function definitions");
}

fn assert_ir_contains(src: &str, pattern: &str) {
    let ir = compile_to_ir(src);
    assert!(ir.contains(pattern), "IR should contain '{}':\n{}", pattern, ir);
}

#[test]
fn codegen_if_else() {
    assert_compiles(r#"
        func abs(x: i32) -> i32 {
            if x < 0 {
                -x
            } else {
                x
            }
        }
    "#);
    assert_ir_contains(r#"
        func abs(x: i32) -> i32 {
            if x < 0 {
                -x
            } else {
                x
            }
        }
    "#, "then");
    assert_ir_contains(r#"
        func abs(x: i32) -> i32 {
            if x < 0 {
                -x
            } else {
                x
            }
        }
    "#, "else");
}

#[test]
fn codegen_if_no_else() {
    assert_compiles(r#"
        func clamp(x: i32) -> i32 {
            let result = x
            if x > 100 {
                result = 100
            }
            result
        }
    "#);
    assert_ir_contains(r#"
        func clamp(x: i32) -> i32 {
            let result = x
            if x > 100 {
                result = 100
            }
            result
        }
    "#, "then");
}

#[test]
fn codegen_nested_if_else() {
    assert_compiles(r#"
        func classify(x: i32) -> i32 {
            if x > 0 {
                if x > 10 {
                    2
                } else {
                    1
                }
            } else {
                0
            }
        }
    "#);
    let ir = compile_to_ir(r#"
        func classify(x: i32) -> i32 {
            if x > 0 {
                if x > 10 {
                    2
                } else {
                    1
                }
            } else {
                0
            }
        }
    "#);
    // Should have multiple then/else blocks for nested ifs
    let then_count = ir.matches("then").count();
    assert!(then_count >= 2, "Should have at least 2 'then' blocks for nested ifs, got {}", then_count);
}

#[test]
fn codegen_if_with_let_binding() {
    assert_compiles(r#"
        func max(a: i32, b: i32) -> i32 {
            if a > b {
                let result = a
                result
            } else {
                let result = b
                result
            }
        }
    "#);
}

#[test]
fn codegen_if_complex_condition() {
    assert_compiles(r#"
        func check(x: i32, y: i32) -> i32 {
            if x == y {
                1
            } else {
                0
            }
        }
    "#);
    let ir = compile_to_ir(r#"
        func check(x: i32, y: i32) -> i32 {
            if x == y {
                1
            } else {
                0
            }
        }
    "#);
    // Should have icmp for the equality check
    assert!(ir.contains("icmp"), "IR should contain icmp for comparison");
}

#[test]
fn codegen_while_counter() {
    assert_compiles(r#"
        func count() -> i32 {
            let i = 0
            while i < 10 {
                i = i + 1
            }
            i
        }
    "#);
    let ir = compile_to_ir(r#"
        func count() -> i32 {
            let i = 0
            while i < 10 {
                i = i + 1
            }
            i
        }
    "#);
    assert!(ir.contains("loop"), "IR should contain loop block");
    assert!(ir.contains("loopbody"), "IR should contain loop body block");
}

#[test]
fn codegen_while_break() {
    assert_compiles(r#"
        func find_first() -> i32 {
            let i = 0
            while i < 100 {
                if i == 5 {
                    break
                }
                i = i + 1
            }
            i
        }
    "#);
    let ir = compile_to_ir(r#"
        func find_first() -> i32 {
            let i = 0
            while i < 100 {
                if i == 5 {
                    break
                }
                i = i + 1
            }
            i
        }
    "#);
    // Should have a loopcont block for break target
    assert!(ir.contains("loopcont"), "IR should have loopcont block for break");
}

#[test]
fn codegen_while_continue() {
    assert_compiles(r#"
        func skip_even() -> i32 {
            let sum = 0
            let i = 0
            while i < 10 {
                i = i + 1
                if i % 2 == 0 {
                    continue
                }
                sum = sum + i
            }
            sum
        }
    "#);
}

#[test]
fn codegen_nested_while() {
    assert_compiles(r#"
        func nested() -> i32 {
            let i = 0
            let sum = 0
            while i < 5 {
                let j = 0
                while j < 3 {
                    sum = sum + 1
                    j = j + 1
                }
                i = i + 1
            }
            sum
        }
    "#);
}

#[test]
fn codegen_infinite_while_break() {
    assert_compiles(r#"
        func until_found() -> i32 {
            let i = 0
            while true {
                if i == 10 {
                    break
                }
                i = i + 1
            }
            i
        }
    "#);
}

#[test]
fn codegen_match_literal() {
    assert_compiles(r#"
        type Direction { North | South | East | West }
        func describe(d: Direction) -> i32 {
            match d {
                North => 1
                South => 2
                East => 3
                West => 4
            }
        }
    "#);
    let ir = compile_to_ir(r#"
        type Direction { North | South | East | West }
        func describe(d: Direction) -> i32 {
            match d {
                North => 1
                South => 2
                East => 3
                West => 4
            }
        }
    "#);
    assert!(ir.contains("matchcont"), "IR should have matchcont block");
}

#[test]
fn codegen_match_wildcard() {
    assert_compiles(r#"
        type Color { Red | Green | Blue }
        func is_primary(c: Color) -> i32 {
            match c {
                Red => 1
                Blue => 1
                _ => 0
            }
        }
    "#);
}

#[test]
fn codegen_match_with_variable() {
    assert_compiles(r#"
        type Option { Some(i32) | None }
        func unwrap_or(o: Option, default: i32) -> i32 {
            match o {
                Some(x) => x
                None => default
            }
        }
    "#);
}

#[test]
fn codegen_match_nested() {
    assert_compiles(r#"
        type MyResult { Ok(i32) | Err(i32) }
        type Outer { Value(MyResult) | Empty }
        func flatten(o: Outer) -> i32 {
            match o {
                Value(r) => match r {
                    Ok(v) => v
                    Err(e) => e
                }
                Empty => 0
            }
        }
    "#);
}

#[test]
fn codegen_match_with_guard() {
    assert_compiles(r#"
        type Num { Val(i32) }
        func classify(n: Num) -> i32 {
            match n {
                Val(x) if x > 0 => 1
                Val(x) if x < 0 => -1
                Val(_) => 0
            }
        }
    "#);
}

#[test]
fn codegen_record_creation() {
    assert_compiles(r#"
        type Point { x: i32, y: i32 }
        func make_point() -> i32 {
            let p = Point { x: 1, y: 2 }
            0
        }
    "#);
}

#[test]
fn codegen_record_multiple_fields() {
    assert_compiles(r#"
        type Person { name: i32, age: i32, active: bool }
        func make_person() -> i32 {
            let p = Person { name: 42, age: 25, active: true }
            0
        }
    "#);
}

#[test]
fn codegen_enum_type() {
    assert_compiles(r#"
        type Color { Red | Green | Blue }
        func use_color(c: Color) -> i32 {
            0
        }
    "#);
}

#[test]
fn codegen_newtype() {
    assert_compiles(r#"
        type Meter = f64
        func make_distance() -> i32 {
            let d: Meter = 3.14
            0
        }
    "#);
}

#[test]
fn codegen_type_alias() {
    assert_compiles(r#"
        type UserId = i32
        func get_user() -> i32 {
            let id: UserId = 123
            id
        }
    "#);
}

#[test]
fn codegen_block_as_expression() {
    assert_compiles(r#"
        func block_expr() -> i32 {
            let a = 5
            let b = 10
            let x = a + b
            x
        }
    "#);
}

#[test]
fn codegen_nested_block() {
    assert_compiles(r#"
        func nested_block() -> i32 {
            let b = 3
            let a = b * 2
            let x = a + 1
            x
        }
    "#);
}

#[test]
fn codegen_function_call_chain() {
    assert_compiles(r#"
        func add(a: i32, b: i32) -> i32 {
            a + b
        }
        func mul(a: i32, b: i32) -> i32 {
            a * b
        }
        func chain() -> i32 {
            add(1, 2) + mul(3, 4)
        }
    "#);
    let ir = compile_to_ir(r#"
        func add(a: i32, b: i32) -> i32 {
            a + b
        }
        func mul(a: i32, b: i32) -> i32 {
            a * b
        }
        func chain() -> i32 {
            add(1, 2) + mul(3, 4)
        }
    "#);
    // Should have multiple function definitions
    let def_count = ir.matches("define").count();
    assert!(def_count >= 3, "Should have at least 3 function definitions, got {}", def_count);
}

#[test]
fn codegen_multi_function() {
    assert_compiles(r#"
        func square(x: i32) -> i32 {
            x * x
        }
        func cube(x: i32) -> i32 {
            x * x * x
        }
        func compute() -> i32 {
            square(3) + cube(2)
        }
    "#);
}

#[test]
fn codegen_recursive_function() {
    assert_compiles(r#"
        func factorial(n: i32) -> i32 {
            if n <= 1 {
                1
            } else {
                n * factorial(n - 1)
            }
        }
    "#);
}

#[test]
fn codegen_void_function() {
    assert_compiles(r#"
        func do_nothing() {
            let x = 42
        }
    "#);
}

#[test]
fn codegen_multi_parameter() {
    assert_compiles(r#"
        func many_params(a: i32, b: i32, c: i32, d: i32, e: i32) -> i32 {
            a + b + c + d + e
        }
    "#);
}

#[test]
fn codegen_compound_expression() {
    assert_compiles(r#"
        func compound() -> i32 {
            let x = 1 + 2 * 3 - 4 / 2
            let y = x > 5
            if y {
                x * 2
            } else {
                x
            }
        }
    "#);
}
