use criterion::{black_box, criterion_group, criterion_main, Criterion};

use mimi::{core, lexer, parser};

fn compile_to_ir(src: &str) -> String {
    let file = parser::Parser::new(lexer::Lexer::new(src).tokenize().unwrap()).parse_file().unwrap();
    core::check(&file).unwrap();
    let context = inkwell::context::Context::create();
    let mut codegen = mimi::codegen::CodeGenerator::new(&context, "bench");
    codegen.compile_file(&file).unwrap();
    codegen.emit_ir()
}

fn codegen_simple(c: &mut Criterion) {
    let src = "func main() -> i32 { 42 }";
    c.bench_function("codegen/simple", |b| {
        b.iter(|| compile_to_ir(black_box(src)))
    });
}

fn codegen_complex(c: &mut Criterion) {
    let src = r#"
type Shape = Circle(f64) | Rect(f64, f64)
func area(s: Shape) -> f64 {
    match s {
        Circle(r) => 3.14159 * r * r,
        Rect(w, h) => w * h,
    }
}
func main() -> f64 {
    area(Circle(5.0)) + area(Rect(3.0, 4.0))
}
"#;
    c.bench_function("codegen/complex", |b| {
        b.iter(|| compile_to_ir(black_box(src)))
    });
}

fn codegen_recursive(c: &mut Criterion) {
    let src = r#"
func fib(n: i32) -> i32 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
func main() -> i32 { fib(20) }
"#;
    c.bench_function("codegen/recursive_fib", |b| {
        b.iter(|| compile_to_ir(black_box(src)))
    });
}

criterion_group!(benches, codegen_simple, codegen_complex, codegen_recursive);
criterion_main!(benches);
