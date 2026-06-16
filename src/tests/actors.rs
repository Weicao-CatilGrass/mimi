use super::*;

#[test]
fn actor_await_method() {
    let src = r#"
actor Counter {
    mut count: i32 = 0;

    func increment() {
        self.count = self.count + 1;
    }

    func get() -> i32 {
        return self.count;
    }
}

func main() -> i32 {
    let c = Counter.spawn();
    c.increment();
    let val = await c.get();
    val
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(1));
}

#[test]
fn actor_sync_method_still_works() {
    let src = r#"
actor Counter {
    mut count: i32 = 0;

    func get() -> i32 {
        return self.count;
    }
}

func main() -> i32 {
    let c = Counter.spawn();
    c.get()
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(0));
}

#[test]
fn actor_await_multiple_methods() {
    let src = r#"
actor Calculator {
    mut value: i32 = 0;

    func add(n: i32) {
        self.value = self.value + n;
    }

    func get() -> i32 {
        return self.value;
    }
}

func main() -> i32 {
    let calc = Calculator.spawn();
    calc.add(10);
    calc.add(20);
    let result = await calc.get();
    result
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Int(30));
}

#[test]
fn actor_await_with_args() {
    let src = r#"
actor Greeter {
    mut name: string = "world";

    func greet() -> string {
        return "Hello, " + self.name;
    }
}

func main() {
    let g = Greeter.spawn();
    let msg = await g.greet();
    println(msg);
}
"#;
    let v = run_source(src);
    assert_eq!(v, interp::Value::Unit);
}
