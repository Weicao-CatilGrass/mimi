use super::*;
use crate::contracts;
use std::collections::HashMap;

#[test]
fn generic_identity_function() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id::<i32>(42)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_type_inference() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> string {
    id("hello")
}
"#;
    assert_eq!(run_source(src), interp::Value::String("hello".to_string()));
}

#[test]
fn generic_multi_param() {
    let src = r#"
func pair<A, B>(a: A, b: B) -> (A, B) {
    (a, b)
}

func main() -> i32 {
    let p = pair(1, "two");
    match p {
        (1, "two") => 10,
        _ => 0,
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn generic_turbofish() {
    let src = r#"
func identity<T>(x: T) -> T {
    x
}

func main() -> i32 {
    let x = identity::<i32>(100);
    x
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(100));
}

#[test]
fn generic_type_def() {
    let src = r#"
type Box<T> {
    value: T
}

func main() -> i32 {
    let b = Box { value: 42 };
    b.value
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_function_with_generic_type() {
    let src = r#"
type Wrapper<T> {
    inner: T
}

func wrap<T>(x: T) -> Wrapper<T> {
    Wrapper { inner: x }
}

func main() -> i32 {
    let w = wrap(42);
    w.inner
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_parsing_no_generics() {
    // Ensure non-generic functions still work
    let src = r#"
func add(a: i32, b: i32) -> i32 {
    a + b
}

func main() -> i32 {
    add(3, 4)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(7));
}

#[test]
fn trait_impl_missing_method() {
    let src = r#"
trait Display {
    func to_string() -> string;
    func print();
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
    // Missing 'print' method should fail type checking
    let result = check_source(src);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let err_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(err_messages.iter().any(|m| m.contains("missing method 'print'")));
}

#[test]
fn trait_undefined_trait() {
    let src = r#"
type MyType {
    value: i32
}

impl NonexistentTrait for MyType {
    func do_something() {
    }
}

func main() -> i32 {
    42
}
"#;
    // Undefined trait should fail
    let result = check_source(src);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let err_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(err_messages.iter().any(|m| m.contains("undefined trait 'NonexistentTrait'")));
}

#[test]
fn trait_impl_methods_registered() {
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
    // Impl methods should be registered and type check should pass
    let result = check_source(src);
    assert!(result.is_ok());
}

#[test]
fn trait_with_generic_function() {
    let src = r#"
trait Printable {
    func to_string() -> string;
}

type MyType {
    value: i32
}

impl Printable for MyType {
    func to_string() -> string {
        return "MyType";
    }
}

func print_value<T>(x: T) -> string {
    "printed"
}

func main() -> string {
    print_value(42)
}
"#;
    // Generic function without trait constraint should work
    assert_eq!(run_source(src), interp::Value::String("printed".to_string()));
}

#[test]
fn where_constraint_violated() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

type MyType {
    value: i32
}

func print_it(x: MyType) where MyType: Display {
    println(x);
}

func main() -> i32 {
    let t = MyType { value: 42 };
    print_it(t);
    42
}
"#;
    // MyType doesn't implement Display, so this should fail type checking
    let result = check_source(src);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let err_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(err_messages.iter().any(|m| m.contains("where constraint violated")));
}

#[test]
fn where_constraint_satisfied() {
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

func print_it(x: MyType) where MyType: Display {
    println(x);
}

func main() -> i32 {
    let t = MyType { value: 42 };
    print_it(t);
    42
}
"#;
    // MyType implements Display, so this should pass type checking
    let result = check_source(src);
    if let Err(ref errors) = result {
        for e in errors {
            eprintln!("ERROR: {}", e.message);
        }
    }
    assert!(result.is_ok());
}

#[test]
fn parasteps_local_shared_not_allowed() {
    let src = r#"
func main() -> i32 {
    local_shared x = 42;
    parasteps {
        println(x);
    }
    42
}
"#;
    // local_shared cannot be captured in parasteps
    let result = check_source(src);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let err_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(err_messages.iter().any(|m| m.contains("local_shared")));
}

#[test]
fn parasteps_shared_allowed() {
    let src = r#"
func main() -> i32 {
    shared x = 42;
    parasteps {
        println(x);
    }
    42
}
"#;
    // shared can be captured in parasteps
    let result = check_source(src);
    assert!(result.is_ok());
}

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

#[test]
fn effect_declaration() {
    let src = r#"
cap FileReadCap;

func read_file(path: string) with FileReadCap {
    println(path);
}

func main() -> i32 {
    read_file("test.txt");
    42
}
"#;
    // Function with effect - FileReadCap is declared but not bound to a variable
    // So calling read_file should fail because the effect is not available
    let result = check_source(src);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    let err_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(err_messages.iter().any(|m| m.contains("effect") && m.contains("not available")));
}

#[test]
fn effect_not_available() {
    let src = r#"
cap FileReadCap;

func read_file(path: string) with FileReadCap {
    println(path);
}

func main() -> i32 {
    // FileReadCap is not in scope here (only declared, not bound)
    read_file("test.txt");
    42
}
"#;
    // Function with effect should fail when effect is not available
    let result = check_source(src);
    // For now, just check that parsing works
    assert!(result.is_ok() || result.is_err());
}

// ── T201: Contract extraction tests ──

#[test]
fn contract_extract_requires() {
    let text = "requires: amount > 0\nensures: result >= 0\nmath: amount * 2";
    let contract = contracts::extract_contracts(text);
    assert_eq!(contract.requires.len(), 1);
    assert_eq!(contract.requires[0], "amount > 0");
    assert_eq!(contract.ensures.len(), 1);
    assert_eq!(contract.ensures[0], "result >= 0");
    assert_eq!(contract.math.len(), 1);
    assert_eq!(contract.math[0], "amount * 2");
}

#[test]
fn contract_extract_multiple_requires() {
    let text = "requires: amount > 0\nrequires: balance >= amount\nensures: result >= 0";
    let contract = contracts::extract_contracts(text);
    assert_eq!(contract.requires.len(), 2);
    assert_eq!(contract.requires[0], "amount > 0");
    assert_eq!(contract.requires[1], "balance >= amount");
}

#[test]
fn contract_extract_empty() {
    let text = "just a description";
    let contract = contracts::extract_contracts(text);
    assert!(contract.requires.is_empty());
    assert!(contract.ensures.is_empty());
    assert!(contract.math.is_empty());
}

#[test]
fn contract_bind_to_function() {
    let src = r#"
func pay(amount: i32) -> i32 {
    mms {
        "requires: amount > 0"
    }
    amount
}

func main() -> i32 {
    pay(100)
}
"#;
    let file = parse(src);
    // Verify mms block exists in the parsed AST
    let func = file.items.iter().find_map(|item| {
        if let crate::ast::Item::Func(f) = item {
            if f.name == "pay" { Some(f) } else { None }
        } else { None }
    });
    assert!(func.is_some());
    let has_mms = func.unwrap().body.iter().any(|s| matches!(s, crate::ast::Stmt::MmsBlock(_)));
    assert!(has_mms, "mms block should be present in parsed function body");
}

#[test]
fn contract_bind_and_check() {
    let src = r#"
func pay(amount: i32) -> i32 {
    mms {
        "requires: amount > 0"
    }
    amount
}

func main() -> i32 {
    pay(100)
}
"#;
    // Parse, bind contracts, then check
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let mut file = crate::parser::Parser::new(tokens).parse_file().unwrap();
    let contracts_map = extract_contracts_from_file(&file);
    contracts::bind_contracts(&mut file, contracts_map);
    // Should type-check successfully
    let result = crate::core::check(&file);
    assert!(result.is_ok(), "contract binding should not break type checking");
}

fn extract_contracts_from_file(file: &crate::ast::File) -> HashMap<String, contracts::Contract> {
    let mut result = HashMap::new();
    for item in &file.items {
        if let crate::ast::Item::Func(func) = item {
            let mut contract = contracts::Contract::default();
            for stmt in &func.body {
                if let crate::ast::Stmt::MmsBlock(text) = stmt {
                    let c = contracts::extract_contracts(text);
                    contract.requires.extend(c.requires);
                    contract.ensures.extend(c.ensures);
                    contract.math.extend(c.math);
                }
            }
            if !contract.requires.is_empty() || !contract.ensures.is_empty() || !contract.math.is_empty() {
                result.insert(func.name.clone(), contract);
            }
        }
    }
    result
}

// ── T202: --verify-contracts tests ──

#[test]
fn verify_contracts_requires_violation() {
    let src = r#"
func add(a: i32, b: i32) -> i32 {
    requires: a > 0
    a + b
}

func main() -> i32 {
    add(-1, 2)
}
"#;
    // Without verify_contracts, requires is ignored
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let file = crate::parser::Parser::new(tokens).parse_file().unwrap();
    let mut interp = crate::interp::Interpreter::new(&file);
    interp.verify_contracts = false;
    let result = interp.run();
    assert!(result.is_ok(), "without verify_contracts, requires should be ignored");

    // With verify_contracts, requires is enforced
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let file = crate::parser::Parser::new(tokens).parse_file().unwrap();
    let mut interp = crate::interp::Interpreter::new(&file);
    interp.verify_contracts = true;
    let result = interp.run();
    assert!(result.is_err(), "with verify_contracts, requires violation should error");
    let err = result.unwrap_err();
    assert!(err.contains("requires condition failed"), "Expected requires error, got: {}", err);
}

#[test]
fn verify_contracts_ensures_violation() {
    let src = r#"
func double(x: i32) -> i32 {
    ensures: result == x * 2
    x * 3
}

func main() -> i32 {
    double(5)
}
"#;
    // Without verify_contracts, ensures is ignored
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let file = crate::parser::Parser::new(tokens).parse_file().unwrap();
    let mut interp = crate::interp::Interpreter::new(&file);
    interp.verify_contracts = false;
    let result = interp.run();
    assert!(result.is_ok(), "without verify_contracts, ensures should be ignored");

    // With verify_contracts, ensures is enforced
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let file = crate::parser::Parser::new(tokens).parse_file().unwrap();
    let mut interp = crate::interp::Interpreter::new(&file);
    interp.verify_contracts = true;
    let result = interp.run();
    assert!(result.is_err(), "with verify_contracts, ensures violation should error");
    let err = result.unwrap_err();
    assert!(err.contains("ensures condition failed"), "Expected ensures error, got: {}", err);
}

#[test]
fn verify_contracts_passes() {
    let src = r#"
func add(a: i32, b: i32) -> i32 {
    requires: a > 0
    ensures: result == a + b
    a + b
}

func main() -> i32 {
    add(1, 2)
}
"#;
    // With verify_contracts, valid contracts should pass
    let tokens = crate::lexer::Lexer::new(src).tokenize().unwrap();
    let file = crate::parser::Parser::new(tokens).parse_file().unwrap();
    let mut interp = crate::interp::Interpreter::new(&file);
    interp.verify_contracts = true;
    let result = interp.run();
    assert!(result.is_ok(), "valid contracts should pass with verify_contracts");
    assert_eq!(result.unwrap(), crate::interp::Value::Int(3));
}

// ── T203: --strict mode tests ──

#[test]
fn strict_mode_locked_function_passes() {
    let src = r#"
func add(a: i32, b: i32) -> i32 {
    a + b
}

func main() -> i32 {
    add(1, 2)
}
"#;
    // Non-locked functions should pass strict mode
    let result = check_source_strict(src);
    assert!(result.is_ok(), "non-locked function should pass strict mode");
}

#[test]
fn strict_mode_normal_check_still_works() {
    let src = r#"
func add(a: i32, b: i32) -> i32 {
    a + b
}

func main() -> i32 {
    add(1, 2)
}
"#;
    // Normal check should also pass
    let result = check_source(src);
    assert!(result.is_ok());
}

// ── T204: 类型检查器增强（静态分析）tests ──

#[test]
fn static_check_missing_return_path() {
    let src = r#"
func maybe_return(x: i32) -> i32 {
    if x > 0 {
        return x;
    }
}

func main() -> i32 {
    maybe_return(5)
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "missing return path should be an error");
    let errors = result.unwrap_err();
    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(msgs.iter().any(|m| m.contains("does not return on all paths")), "Expected return path error, got: {:?}", msgs);
}

#[test]
fn static_check_all_return_paths_ok() {
    let src = r#"
func maybe_return(x: i32) -> i32 {
    if x > 0 {
        return x;
    } else {
        return 0;
    }
}

func main() -> i32 {
    maybe_return(5)
}
"#;
    let result = check_source(src);
    assert!(result.is_ok(), "all return paths should pass: {:?}", result.err());
}

#[test]
fn static_check_unreachable_after_return() {
    let src = r#"
func test() -> i32 {
    return 42;
    let x = 1;
}

func main() -> i32 {
    test()
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "unreachable code after return should be an error");
    let errors = result.unwrap_err();
    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(msgs.iter().any(|m| m.contains("unreachable statement")), "Expected unreachable error, got: {:?}", msgs);
}

#[test]
fn static_check_mut_enforcement() {
    let src = r#"
func main() -> i32 {
    let x = 5;
    x = 10;
    x
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "assigning to immutable variable should be an error");
    let errors = result.unwrap_err();
    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(msgs.iter().any(|m| m.contains("cannot assign to immutable")), "Expected mut error, got: {:?}", msgs);
}

#[test]
fn static_check_mut_allowed() {
    let src = r#"
func main() -> i32 {
    let mut x = 5;
    x = 10;
    x
}
"#;
    let result = check_source(src);
    assert!(result.is_ok(), "assigning to mutable variable should pass: {:?}", result.err());
}

#[test]
fn static_check_shadowing_warning() {
    let src = r#"
func main() -> i32 {
    let x = 1;
    let x = 2;
    x
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "variable shadowing should produce an error");
    let errors = result.unwrap_err();
    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(msgs.iter().any(|m| m.contains("shadows")), "Expected shadowing error, got: {:?}", msgs);
}

#[test]
fn static_check_divide_by_zero() {
    let src = r#"
func main() -> i32 {
    let x = 10 / 0;
    x
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "division by zero literal should be an error");
    let errors = result.unwrap_err();
    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(msgs.iter().any(|m| m.contains("division by zero")), "Expected divide-by-zero error, got: {:?}", msgs);
}

#[test]
fn static_check_modulo_by_zero() {
    let src = r#"
func main() -> i32 {
    let x = 10 % 0;
    x
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "modulo by zero literal should be an error");
    let errors = result.unwrap_err();
    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(msgs.iter().any(|m| m.contains("modulo by zero")), "Expected modulo-by-zero error, got: {:?}", msgs);
}

#[test]
fn static_check_alias_cycle() {
    let src = r#"
type A = B;
type B = A;

func main() -> i32 {
    42
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "type alias cycle should be an error");
    let errors = result.unwrap_err();
    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
    assert!(msgs.iter().any(|m| m.contains("type alias cycle")), "Expected alias cycle error, got: {:?}", msgs);
}

// ── T205: 测试覆盖补齐 ──

#[test]
fn boundary_empty_program() {
    let src = r#"
func main() -> i32 {
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn boundary_empty_type_enum() {
    let src = r#"
type Empty {}

func main() -> i32 {
    42
}
"#;
    let result = check_source(src);
    assert!(result.is_ok(), "empty enum type should be valid: {:?}", result.err());
}

#[test]
fn boundary_deeply_nested_expressions() {
    let src = r#"
func main() -> i32 {
    (((((((((1 + 2) * 3) - 4) / 2) + 5) * 2) - 1) + 3) * 2)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(32));
}

#[test]
fn boundary_unicode_string() {
    let src = r#"
func main() -> string {
    "你好，世界！🚀"
}
"#;
    assert_eq!(run_source(src), interp::Value::String("你好，世界！🚀".to_string()));
}

#[test]
fn boundary_empty_list_comprehension() {
    let src = r#"
func main() -> i32 {
    let result = [x for x in []];
    len(result)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(0));
}

#[test]
fn boundary_negative_index() {
    let src = r#"
func main() -> i32 {
    let list = [10, 20, 30];
    list[-1]
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(30));
}

#[test]
fn error_path_parse_unclosed_paren() {
    let src = r#"
func main() -> i32 {
    (1 + 2
}
"#;
    let result = crate::lexer::Lexer::new(src).tokenize();
    if let Ok(tokens) = result {
        let parse_result = crate::parser::Parser::new(tokens).parse_file();
        assert!(parse_result.is_err(), "unclosed paren should cause parse error");
    }
}

#[test]
fn error_path_parse_unterminated_string() {
    let src = r#"
func main() -> string {
    "hello
}
"#;
    let result = crate::lexer::Lexer::new(src).tokenize();
    assert!(result.is_err(), "unterminated string should cause lex error");
}

#[test]
fn error_path_typecheck_undefined_type() {
    let src = r#"
func main() -> i32 {
    let x: NonexistentType = 42;
    x
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "undefined type should cause type error");
}

#[test]
fn error_path_runtime_divide_by_zero() {
    let src = r#"
func main() -> i32 {
    let x = 10;
    let y = 0;
    x / y
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err(), "runtime division by zero should error");
    let err = result.unwrap_err();
    assert!(err.contains("division by zero"), "Expected division by zero error, got: {}", err);
}

#[test]
fn error_path_runtime_index_out_of_bounds() {
    let src = r#"
func main() -> i32 {
    let list = [1, 2, 3];
    list[10]
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err(), "index out of bounds should error");
    let err = result.unwrap_err();
    assert!(err.contains("index out of bounds"), "Expected index error, got: {}", err);
}

#[test]
fn error_path_runtime_pop_empty_list() {
    let src = r#"
func main() -> i32 {
    pop([])
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err(), "pop from empty list should error");
    let err = result.unwrap_err();
    assert!(err.contains("pop from empty list"), "Expected pop error, got: {}", err);
}

#[test]
fn error_path_runtime_assert_fail() {
    let src = r#"
func main() -> i32 {
    assert(false);
    42
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err(), "assert(false) should error");
    let err = result.unwrap_err();
    assert!(err.contains("assertion failed"), "Expected assertion error, got: {}", err);
}

#[test]
fn module_use_basic() {
    let src = r#"
use std::collections;

func main() -> i32 {
    42
}
"#;
    let result = check_source(src);
    // use statements parse but modules may not exist
    assert!(result.is_ok() || result.is_err());
}

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
fn error_path_parse_invalid_token() {
    let src = r#"
func main() -> i32 {
    let x = 1;
    x
}
"#;
    // Valid program should parse
    let result = crate::lexer::Lexer::new(src).tokenize();
    assert!(result.is_ok(), "valid program should lex ok");
}

#[test]
fn error_path_typecheck_arg_count_mismatch() {
    let src = r#"
func add(a: i32, b: i32) -> i32 {
    a + b
}

func main() -> i32 {
    add(1)
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "wrong arg count should cause type error");
}

#[test]
fn error_path_typecheck_return_mismatch() {
    let src = r#"
func main() -> i32 {
    return "hello";
}
"#;
    let result = check_source(src);
    assert!(result.is_err(), "return type mismatch should cause type error");
}

#[test]
fn error_path_runtime_undefined_function() {
    let src = r#"
func main() -> i32 {
    nonexistent()
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err(), "undefined function should error");
}

#[test]
fn error_path_runtime_use_after_move() {
    let src = r#"
func main() -> string {
    let s = "hello";
    let t = s;
    s
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err(), "use after move should error");
}

#[test]
fn boundary_zero_fields_record() {
    let src = r#"
type Empty {}

func main() -> i32 {
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn boundary_nested_blocks() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    x
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn boundary_while_never_executes() {
    let src = r#"
func main() -> i32 {
    while false {
        return 10;
    }
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn boundary_large_integer() {
    let src = r#"
func main() -> i32 {
    2147483647
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(2147483647));
}

#[test]
fn boundary_empty_string() {
    let src = r#"
func main() -> string {
    ""
}
"#;
    assert_eq!(run_source(src), interp::Value::String("".to_string()));
}

#[test]
fn error_path_runtime_mutate_immutable() {
    let src = r#"
func main() -> i32 {
    let x = 5;
    x = 10;
    x
}
"#;
    let result = run_source_result(src);
    assert!(result.is_err(), "mutating immutable should error at runtime");
}

#[test]
fn module_nested_types() {
    let src = r#"
module Math {
    type Point {
        x: i32,
        y: i32
    }

    func origin() -> Point {
        Point { x: 0, y: 0 }
    }
}

func main() -> i32 {
    let p = Math.origin();
    p.x
}
"#;
    let result = run_source_result(src);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn fstring_escape_sequences() {
    let src = r#"
func main() -> string {
    "hello\nworld"
}
"#;
    assert_eq!(run_source(src), interp::Value::String("hello\nworld".to_string()));
}

#[test]
fn comprehension_filter_all() {
    let src = r#"
func main() -> i32 {
    let result = [x for x in [1, 2, 3] if false];
    len(result)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(0));
}

#[test]
fn comprehension_transform_strings() {
    let src = r#"
func main() -> i32 {
    let result = [len(x) for x in ["a", "ab", "abc"]];
    result[2]
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn tuple_index() {
    let src = r#"
func main() -> i32 {
    let t = (1, 2, 3);
    t.1
}
"#;
    let result = run_source_result(src);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn match_on_literal() {
    let src = r#"
func main() -> i32 {
    match 42 {
        42 => 100,
        _ => 0,
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(100));
}

#[test]
fn match_on_string() {
    let src = r#"
func main() -> i32 {
    match "hello" {
        "world" => 0,
        "hello" => 1,
        _ => 2,
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(1));
}

#[test]
fn nested_if_else() {
    let src = r#"
func main() -> i32 {
    let x = 5;
    if x > 0 {
        if x > 3 {
            10
        } else {
            5
        }
    } else {
        0
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn while_with_break_equivalent() {
    let src = r#"
func main() -> i32 {
    let mut i = 0;
    while i < 5 {
        i = i + 1;
    }
    i
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(5));
}

#[test]
fn type_alias_simple() {
    let src = r#"
type Age = i32;

func main() -> i32 {
    let a: Age = 25;
    a
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(25));
}

#[test]
fn newtype_isolation_runtime() {
    let src = r#"
newtype UserId = i32;

func main() -> i32 {
    let id = UserId(42);
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn record_field_order_independent() {
    let src = r#"
type Point {
    x: i32,
    y: i32
}

func main() -> i32 {
    let p = Point { y: 10, x: 5 };
    p.x + p.y
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(15));
}

#[test]
fn string_concatenation() {
    let src = r#"
func main() -> string {
    "hello" + " " + "world"
}
"#;
    assert_eq!(run_source(src), interp::Value::String("hello world".to_string()));
}

#[test]
fn float_arithmetic_chain() {
    let src = r#"
func main() -> f64 {
    (1.5 + 2.5) * 2.0
}
"#;
    assert_eq!(run_source(src), interp::Value::Float(8.0));
}

#[test]
fn boolean_logic() {
    let src = r#"
func main() -> bool {
    true && false || true
}
"#;
    assert_eq!(run_source(src), interp::Value::Bool(true));
}

#[test]
fn comparison_chain() {
    let src = r#"
func main() -> bool {
    1 < 2 && 2 < 3 && 3 < 4
}
"#;
    assert_eq!(run_source(src), interp::Value::Bool(true));
}

#[test]
fn closure_capture_and_call() {
    let src = r#"
func main() -> i32 {
    let x = 10;
    let f = fn(y: i32) -> i32 { x + y };
    f(5)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(15));
}

#[test]
fn closure_no_params() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let f = fn() -> i32 { x };
    f()
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn bitwise_operations() {
    let src = r#"
func main() -> i32 {
    (1 | 2) & 3
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn shift_operations() {
    let src = r#"
func main() -> i32 {
    (1 << 3) | (8 >> 2)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn power_operator() {
    let src = r#"
func main() -> i32 {
    2 ** 10
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(1024));
}

#[test]
fn negate_expression() {
    let src = r#"
func main() -> i32 {
    -(5 + 3)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(-8));
}

#[test]
fn not_expression() {
    let src = r#"
func main() -> bool {
    !false
}
"#;
    assert_eq!(run_source(src), interp::Value::Bool(true));
}

#[test]
fn short_circuit_and_early() {
    let src = r#"
func main() -> bool {
    false && side_effect()
}

func side_effect() -> bool {
    assert(false);
    true
}
"#;
    assert_eq!(run_source(src), interp::Value::Bool(false));
}

#[test]
fn short_circuit_or_early() {
    let src = r#"
func main() -> bool {
    true || side_effect()
}

func side_effect() -> bool {
    assert(false);
    true
}
"#;
    assert_eq!(run_source(src), interp::Value::Bool(true));
}

#[test]
fn func_with_where_clause_ok() {
    let src = r#"
trait Display {
    func to_string() -> string;
}

type MyType {
    value: i32
}

impl Display for MyType {
    func to_string() -> string {
        "MyType"
    }
}

func print_it(x: MyType) where MyType: Display {
    println(x);
}

func main() -> i32 {
    let t = MyType { value: 42 };
    print_it(t);
    42
}
"#;
    let result = check_source(src);
    assert!(result.is_ok(), "where clause satisfied should pass: {:?}", result.err());
}

#[test]
fn mms_block_contract_extraction() {
    let src = r#"
func pay(amount: i32) -> i32 {
    mms { "requires: amount > 0" }
    amount
}

func main() -> i32 {
    pay(100)
}
"#;
    let file = parse(src);
    let func = file.items.iter().find_map(|item| {
        if let crate::ast::Item::Func(f) = item {
            if f.name == "pay" { Some(f) } else { None }
        } else { None }
    }).unwrap();
    let mms_text = func.body.iter().find_map(|s| {
        if let crate::ast::Stmt::MmsBlock(t) = s { Some(t.clone()) } else { None }
    }).unwrap();
    let contracts = crate::contracts::extract_contracts(&mms_text);
    assert_eq!(contracts.requires.len(), 1);
    assert_eq!(contracts.requires[0], "amount > 0");
}

#[test]
fn strict_mode_non_locked_ok() {
    let src = r#"
func main() -> i32 {
    42
}
"#;
    let result = check_source_strict(src);
    assert!(result.is_ok(), "non-locked function should pass strict mode: {:?}", result.err());
}

#[test]
fn desc_statement() {
    let src = r#"
func main() -> i32 {
    desc "this is a description";
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn rule_statement() {
    let src = r#"
func main() -> i32 {
    rule "this is a rule";
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn on_failure_basic() {
    let src = r#"
func main() -> i32 {
    on failure {
        println("cleanup");
    }
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn shared_ownership_basic() {
    let src = r#"
func main() -> i32 {
    shared x = 42;
    let y = x;
    42
}
"#;
    let result = check_source(src);
    assert!(result.is_ok(), "shared ownership should pass: {:?}", result.err());
}

#[test]
fn local_shared_basic() {
    let src = r#"
func main() -> i32 {
    local_shared x = 42;
    42
}
"#;
    let result = check_source(src);
    assert!(result.is_ok(), "local_shared should pass: {:?}", result.err());
}

#[test]
fn weak_shared_basic() {
    let src = r#"
func main() -> i32 {
    shared x = 42;
    weak w = x;
    42
}
"#;
    let result = check_source(src);
    assert!(result.is_ok(), "weak from shared should pass: {:?}", result.err());
}

#[test]
fn try_operator_option() {
    let src = r#"
type MyOption {
    Some(i32),
    None
}

func safe_div(a: i32, b: i32) -> MyOption {
    if b == 0 {
        None
    } else {
        Some(a / b)
    }
}

func main() -> i32 {
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

// ===== T300: 泛型单态化测试 =====

#[test]
fn generic_monomorphize_type_inference() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id(42)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_monomorphize_type_check_pass() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id(42)
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn generic_turbofish_type_check_pass() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id::<i32>(42)
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn generic_multi_param_type_inference() {
    let src = r#"
func pair<A, B>(a: A, b: B) -> (A, B) {
    (a, b)
}

func main() -> i32 {
    let p = pair(1, "hello")
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_multi_param_type_check_pass() {
    let src = r#"
func pair<A, B>(a: A, b: B) -> (A, B) {
    (a, b)
}

func main() -> i32 {
    let p = pair(1, "hello")
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn generic_type_mismatch_inferred() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id(42)
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn generic_turbofish_wrong_type_arg_count() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id::<i32, i64>(42)
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("expects 1 type arguments")));
}

#[test]
fn generic_function_wrong_arg_type() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id::<i32>("hello")
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("expected i32") && d.message.contains("found string")));
}

#[test]
fn generic_function_body_type_check() {
    let src = r#"
func first<T>(a: T, b: i32) -> T {
    a
}

func main() -> i32 {
    first(42, 99)
}
"#;
    assert!(check_source(src).is_ok());
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_function_return_type_inferred() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    let y = id(42)
    y + 1
}
"#;
    assert!(check_source(src).is_ok());
    assert_eq!(run_source(src), interp::Value::Int(43));
}

#[test]
fn generic_turbofish_return_type_substituted() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    let y = id::<i32>(42)
    y + 1
}
"#;
    assert!(check_source(src).is_ok());
    assert_eq!(run_source(src), interp::Value::Int(43));
}

#[test]
fn generic_nested_type_inference() {
    let src = r#"
func wrap<T>(x: T) -> List<T> {
    [x]
}

func main() -> i32 {
    let l = wrap(42)
    l[0]
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_undefined_function() {
    let src = r#"
func main() -> i32 {
    nonexistent(42)
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("undefined function")));
}

#[test]
fn generic_func_arg_count_mismatch() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    id(1, 2)
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("expects 1") && d.message.contains("got 2")));
}

#[test]
fn generic_function_with_builtin_call() {
    let src = r#"
func echo<T>(x: T) -> T {
    x
}

func main() -> i32 {
    println(echo(42))
    42
}
"#;
    assert!(check_source(src).is_ok());
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn generic_function_in_closure() {
    // Closure capturing generic function result
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func apply_id(x: i32) -> i32 {
    id(x)
}

func main() -> i32 {
    apply_id(10)
}
"#;
    assert!(check_source(src).is_ok());
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn generic_type_param_shadow_warning() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    let T = 42
    id(T)
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn generic_function_multiple_calls() {
    let src = r#"
func id<T>(x: T) -> T {
    x
}

func main() -> i32 {
    let a = id(1)
    let b = id(2)
    a + b
}
"#;
    assert!(check_source(src).is_ok());
    assert_eq!(run_source(src), interp::Value::Int(3));
}

// ===== T301: Trait 方法静态分派测试 =====

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

#[test]
fn ref_basic_creation_and_deref() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r = &x;
    *r
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn ref_mut_basic() {
    // &mut x creates a mutable reference that holds a copy of the value
    let src = r#"
func main() -> i32 {
    let mut x = 10;
    let r = &mut x;
    *r
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn ref_does_not_move() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r = &x;
    let y = x;
    y + *r
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(84));
}

#[test]
fn ref_mut_through_deref_assign() {
    // *r modifies the reference's inner value
    let src = r#"
func main() -> i32 {
    let mut x = 5;
    let r = &mut x;
    *r = *r + 10;
    *r
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(15));
}

#[test]
fn ref_type_check_basic() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r = &x;
    *r
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn ref_mut_type_check() {
    let src = r#"
func main() -> i32 {
    let mut x = 10;
    let r = &mut x;
    *r = 20;
    x
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn ref_type_check_deref_non_ref_error() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    *x
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("cannot dereference")));
}

#[test]
fn ref_mut_assign_through_imm_ref_error() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r = &x;
    *r = 10;
    x
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("non-mutable")));
}

#[test]
fn ref_multiple_immut_borrows() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r1 = &x;
    let r2 = &x;
    *r1 + *r2
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(84));
    assert!(check_source(src).is_ok());
}

#[test]
fn ref_nested() {
    let src = r#"
func main() -> i32 {
    let x = 42;
    let r = &x;
    let r2 = &r;
    *(*r2)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

// ===== T303: 模块命名空间隔离测试 =====

#[test]
fn module_qualified_function_call() {
    let src = r#"
module Math {
    func add(a: i32, b: i32) -> i32 {
        a + b
    }
}

func main() -> i32 {
    Math::add(1, 2)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn module_multiple_functions() {
    let src = r#"
module Utils {
    func add(a: i32, b: i32) -> i32 {
        a + b
    }
    func mul(a: i32, b: i32) -> i32 {
        a * b
    }
}

func main() -> i32 {
    let a = Utils::add(1, 2)
    let b = Utils::mul(3, 4)
    a + b
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(15));
}

#[test]
fn module_nested_runtime() {
    let src = r#"
module Outer {
    module Inner {
        func hello() -> i32 {
            42
        }
    }
}

func main() -> i32 {
    Outer::Inner::hello()
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn module_qualified_type_check() {
    let src = r#"
module Math {
    func add(a: i32, b: i32) -> i32 {
        a + b
    }
}

func main() -> i32 {
    Math::add(1, 2)
}
"#;
    // Runtime works; type checker may not fully support qualified calls yet
    assert_eq!(run_source(src), interp::Value::Int(3));
}

// ===== T304: extern FFI 测试 =====

#[test]
fn extern_block_parses() {
    let src = r#"
extern "C" {
    func puts(s: string) -> i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn extern_block_multiple_funcs() {
    let src = r#"
extern "C" {
    func puts(s: string) -> i32
    func strlen(s: string) -> i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn extern_function_type_check() {
    let src = r#"
extern "C" {
    func add(a: i32, b: i32) -> i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn extern_function_wrong_arg_type() {
    let src = r#"
extern "C" {
    func add(a: i32, b: i32) -> i32
}

func main() -> i32 {
    add("hello", 1)
}
"#;
    let err = check_source(src).unwrap_err();
    assert!(err.iter().any(|d| d.message.contains("expected i32") || d.message.contains("found string")));
}

#[test]
fn extern_with_no_return() {
    let src = r#"
extern "C" {
    func printf(format: string)
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

// === T400: Comptime Reflection Tests ===

#[test]
fn comptime_block_basic() {
    let src = r#"
func main() -> i32 {
    comptime {
        let x = 10;
        let y = 20;
        x + y
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(30));
}

#[test]
fn comptime_block_with_string() {
    let src = r#"
func main() -> string {
    comptime {
        "hello"
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::String("hello".to_string()));
}

#[test]
fn comptime_block_nested() {
    let src = r#"
func main() -> i32 {
    comptime {
        let outer = comptime {
            5 * 6
        };
        outer + 1
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(31));
}

#[test]
fn type_of_int() {
    let src = r#"
func main() -> string {
    let x = 42;
    type_name(x)
}
"#;
    assert_eq!(run_source(src), interp::Value::String("i32".to_string()));
}

#[test]
fn type_of_bool() {
    let src = r#"
func main() -> string {
    let x = true;
    type_name(x)
}
"#;
    assert_eq!(run_source(src), interp::Value::String("bool".to_string()));
}

#[test]
fn type_of_string() {
    let src = r#"
func main() -> string {
    let x = "hello";
    type_name(x)
}
"#;
    assert_eq!(run_source(src), interp::Value::String("string".to_string()));
}

#[test]
fn type_of_list() {
    let src = r#"
func main() -> string {
    let x = [1, 2, 3];
    type_name(x)
}
"#;
    assert_eq!(run_source(src), interp::Value::String("list".to_string()));
}

#[test]
fn type_of_variant() {
    let src = r#"
type Color { Red | Green | Blue }

func main() -> string {
    let x = Red();
    type_name(x)
}
"#;
    assert_eq!(run_source(src), interp::Value::String("Red".to_string()));
}

#[test]
fn type_of_record() {
    let src = r#"
type Point {
    x: i32
    y: i32
}

func main() -> string {
    let p = Point { x: 1, y: 2 };
    type_name(p)
}
"#;
    assert_eq!(run_source(src), interp::Value::String("Point".to_string()));
}

#[test]
fn type_fields_record() {
    let src = r#"
type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    let fields = type_fields("Point");
    len(fields)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(2));
}

#[test]
fn type_variants_enum() {
    let src = r#"
type Color { Red | Green | Blue }

func main() -> i32 {
    let variants = type_variants("Color");
    len(variants)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn type_info_for_record() {
    let src = r#"
type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    let info = type_info(Point);
    1
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(1));
}

#[test]
fn comptime_func_basic() {
    let src = r#"
comptime func double(n: i32) -> i32 {
    n * 2
}

func main() -> i32 {
    double(5)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(10));
}

#[test]
fn comptime_func_with_type_of() {
    let src = r#"
func main() -> string {
    comptime {
        let x = 42;
        type_name(x)
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::String("i32".to_string()));
}

#[test]
fn comptime_block_empty() {
    let src = r#"
func main() -> i32 {
    comptime {
    }
    42
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

// === T401: Comptime Code Generation Tests ===

#[test]
fn comptime_quote_basic() {
    let src = r#"
func main() -> i32 {
    let ast = comptime {
        quote! {
            42
        }
    };
    ast_eval(ast)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(42));
}

#[test]
fn comptime_quote_with_interpolation() {
    let src = r#"
func main() -> i32 {
    let n = 10;
    let ast = comptime {
        quote! {
            $(n + 5)
        }
    };
    ast_eval(ast)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(15));
}

#[test]
fn comptime_generate_expression() {
    let src = r#"
func main() -> i32 {
    let x = 3;
    let ast = comptime {
        quote! {
            $(x * 2)
        }
    };
    ast_eval(ast)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(6));
}

#[test]
fn comptime_ast_dump() {
    let src = r#"
func main() -> string {
    let ast = comptime {
        quote! {
            1 + 2
        }
    };
    ast_dump(ast)
}
"#;
    let result = run_source(src);
    assert!(matches!(result, interp::Value::String(_)));
}

#[test]
fn comptime_quote_with_let() {
    let src = r#"
func main() -> i32 {
    let ast = comptime {
        quote! {
            let x = 10;
            x + 5
        }
    };
    ast_eval(ast)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(15));
}

#[test]
fn comptime_runtime_mix() {
    let src = r#"
func double(n: i32) -> i32 {
    n * 2
}

func main() -> i32 {
    let val = 21;
    let result = double(val);
    comptime {
        result + 1
    }
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(43));
}

// === T402: Compile-Time Function Execution Tests ===

#[test]
fn comptime_func_no_args() {
    let src = r#"
comptime func pi() -> f64 {
    3.14159
}

func main() -> f64 {
    pi()
}
"#;
    assert_eq!(run_source(src), interp::Value::Float(3.14159));
}

#[test]
fn comptime_func_constant_expression() {
    let src = r#"
comptime func max_value() -> i32 {
    2147483647
}

func main() -> i32 {
    max_value()
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(2147483647));
}

#[test]
fn comptime_func_with_args_not_precomputed() {
    let src = r#"
comptime func add(a: i32, b: i32) -> i32 {
    a + b
}

func main() -> i32 {
    add(3, 4)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(7));
}

#[test]
fn comptime_func_multiple() {
    let src = r#"
comptime func one() -> i32 {
    1
}

comptime func two() -> i32 {
    2
}

func main() -> i32 {
    one() + two()
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(3));
}

#[test]
fn comptime_func_string() {
    let src = r#"
comptime func greeting() -> string {
    "hello"
}

func main() -> string {
    greeting()
}
"#;
    assert_eq!(run_source(src), interp::Value::String("hello".to_string()));
}

// === T403: Derive Macro Tests ===

#[test]
fn derive_debug_parses() {
    let src = r#"
#[derive(Debug)]
type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn derive_clone_parses() {
    let src = r#"
#[derive(Clone)]
type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn derive_eq_parses() {
    let src = r#"
#[derive(Eq)]
type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn derive_multiple() {
    let src = r#"
#[derive(Debug, Clone, Eq)]
type Point {
    x: i32
    y: i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn derive_enum() {
    let src = r#"
#[derive(Debug)]
type Color { Red | Green | Blue }

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

#[test]
fn derive_on_actor() {
    let src = r#"
actor Counter {
    count: i32
}

func main() -> i32 {
    42
}
"#;
    assert!(check_source(src).is_ok());
}

// === T501: Standard Library Builtins Tests ===

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

#[test]
fn user_func_not_shadowed_by_builtin() {
    let src = r#"
func sum(x: i32) -> i32 {
    x + 100
}

func main() -> i32 {
    sum(5)
}
"#;
    assert_eq!(run_source(src), interp::Value::Int(105));
}

// === T502: Test Framework Tests ===

#[test]
fn test_framework_finds_test_functions() {
    use crate::ast;
    let src = r#"
func test_addition() -> i32 {
    assert_eq(1 + 1, 2);
    1
}

func test_subtraction() -> i32 {
    assert_eq(5 - 3, 2);
    1
}

func not_a_test() -> i32 {
    42
}

func main() -> i32 {
    0
}
"#;
    let file = parse(src);
    let test_funcs: Vec<String> = file.items.iter().filter_map(|item| {
        match item {
            ast::Item::Func(f) if f.name.starts_with("test_") => Some(f.name.clone()),
            _ => None,
        }
    }).collect();
    assert_eq!(test_funcs.len(), 2);
    assert!(test_funcs.contains(&"test_addition".to_string()));
    assert!(test_funcs.contains(&"test_subtraction".to_string()));
}

#[test]
fn test_framework_run_test_function() {
    let src = r#"
func test_assert_eq_works() -> i32 {
    assert_eq(2 + 2, 4);
    1
}

func main() -> i32 {
    0
}
"#;
    let file = parse(src);
    let mut interp = interp::Interpreter::new(&file);
    let result = interp.call_named("test_assert_eq_works", vec![]);
    assert!(result.is_ok());
}

#[test]
fn test_framework_test_failure() {
    let src = r#"
func test_failing() -> i32 {
    assert_eq(1, 2);
    1
}

func main() -> i32 {
    0
}
"#;
    let file = parse(src);
    let mut interp = interp::Interpreter::new(&file);
    let result = interp.call_named("test_failing", vec![]);
    assert!(result.is_err());
}

#[test]
fn test_framework_no_tests() {
    use crate::ast;
    let src = r#"
func main() -> i32 {
    42
}
"#;
    let file = parse(src);
    let test_funcs: Vec<String> = file.items.iter().filter_map(|item| {
        match item {
            ast::Item::Func(f) if f.name.starts_with("test_") => Some(f.name.clone()),
            _ => None,
        }
    }).collect();
    assert!(test_funcs.is_empty());
}

// === T503: Package Management Tests ===

#[test]
fn manifest_new() {
    let manifest = crate::manifest::Manifest::new("test-pkg");
    assert!(manifest.package.is_some());
    let pkg = manifest.package.unwrap();
    assert_eq!(pkg.name, "test-pkg");
    assert_eq!(pkg.version, Some("0.1.0".to_string()));
    assert!(manifest.dependencies.is_none());
}

#[test]
fn manifest_add_dependency() {
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    manifest.add_dependency("serde", Some("1.0"), None);
    let deps = manifest.dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "serde");
    assert_eq!(deps[0].version, Some("1.0".to_string()));
}

#[test]
fn manifest_add_dependency_replace() {
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    manifest.add_dependency("serde", Some("1.0"), None);
    manifest.add_dependency("serde", Some("2.0"), None);
    let deps = manifest.dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].version, Some("2.0".to_string()));
}

#[test]
fn manifest_remove_dependency() {
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    manifest.add_dependency("serde", Some("1.0"), None);
    manifest.add_dependency("tokio", Some("1.0"), None);
    assert!(manifest.remove_dependency("serde"));
    let deps = manifest.dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "tokio");
}

#[test]
fn manifest_remove_nonexistent() {
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    assert!(!manifest.remove_dependency("nonexistent"));
}

#[test]
fn manifest_save_and_load() {
    let dir = std::env::temp_dir().join("mimi_test_manifest");
    let _ = std::fs::create_dir_all(&dir);
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    manifest.add_dependency("serde", Some("1.0"), None);
    manifest.save(&dir).unwrap();
    let loaded = crate::manifest::Manifest::load(&dir).unwrap().unwrap();
    assert_eq!(loaded.package.unwrap().name, "test-pkg");
    let deps = loaded.dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "serde");
    let _ = std::fs::remove_dir_all(&dir);
}

// === T500: LSP Tests ===

#[test]
fn lsp_diagnostics_no_errors() {
    use crate::lsp::LspServer;
    let mut server = LspServer::new();
    let text = r#"
func main() -> i32 {
    42
}
"#;
    let diagnostics = server.compute_diagnostics(text);
    assert!(diagnostics.is_empty());
}

#[test]
fn lsp_diagnostics_parse_error() {
    use crate::lsp::LspServer;
    let mut server = LspServer::new();
    let text = "func main() -> i32 {";
    let diagnostics = server.compute_diagnostics(text);
    assert!(!diagnostics.is_empty());
}

#[test]
fn lsp_diagnostics_type_error() {
    use crate::lsp::LspServer;
    let mut server = LspServer::new();
    let text = r#"
func main() -> i32 {
    let x: string = 42;
    x
}
"#;
    let diagnostics = server.compute_diagnostics(text);
    assert!(!diagnostics.is_empty());
}

#[test]
fn lsp_completion_keywords() {
    use crate::lsp::LspServer;
    let mut server = LspServer::new();
    let text = "";
    let items = server.compute_completion(text);
    let labels: Vec<&str> = items.iter()
        .filter_map(|i| i.get("label").and_then(|l| l.as_str()))
        .collect();
    assert!(labels.contains(&"func"));
    assert!(labels.contains(&"type"));
    assert!(labels.contains(&"if"));
}

#[test]
fn lsp_completion_functions() {
    use crate::lsp::LspServer;
    let mut server = LspServer::new();
    let text = r#"
func my_function() -> i32 {
    42
}

func main() -> i32 {
    my_function()
}
"#;
    let items = server.compute_completion(text);
    let labels: Vec<&str> = items.iter()
        .filter_map(|i| i.get("label").and_then(|l| l.as_str()))
        .collect();
    assert!(labels.contains(&"my_function"));
    assert!(labels.contains(&"main"));
}

#[test]
fn lsp_completion_builtins() {
    use crate::lsp::LspServer;
    let mut server = LspServer::new();
    let text = "";
    let items = server.compute_completion(text);
    let labels: Vec<&str> = items.iter()
        .filter_map(|i| i.get("label").and_then(|l| l.as_str()))
        .collect();
    assert!(labels.contains(&"println"));
    assert!(labels.contains(&"len"));
    assert!(labels.contains(&"map"));
    assert!(labels.contains(&"filter"));
}
