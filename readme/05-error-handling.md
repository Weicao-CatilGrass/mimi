# 05 - Mimi 错误处理

---

## 1. 总览

Mimi 提供统一的错误处理模型，融合了 Result、Option、? 运算符和 on failure 补偿：

| 机制 | 适用场景 |
|------|----------|
| `Result<T, E>` + `?` | 可恢复错误 |
| `T?` / `Option<T>` | 可选值 |
| `error "msg"` | 不可恢复 / 终止 |
| `on failure { }` | 事务性补偿 |
| `drop(result)` | 显式忽略 |

---

## 2. Result 类型

### 2.1 定义

```mimi
type Result<T, E> {
    Ok(T)
    Err(E)
}
```

### 2.2 创建 Result

```mimi
func divide(a: f64, b: f64) -> Result<f64, string> {
    if b == 0.0 {
        Err("division by zero")
    } else {
        Ok(a / b)
    }
}
```

### 2.3 使用 Result

```mimi
match divide(10.0, 3.0) {
    Ok(result) => println(result),
    Err(msg) => println("Error: ", msg)
}
```

---

## 3. ? 运算符

### 3.1 基本用法

`?` 在 Result 为 Err 时提前返回错误：

```mimi
func parse_and_double(s: string) -> Result<i64, string> {
    let n = parse_int(s)?;   // 失败时返回 Err
    Ok(n * 2)
}
```

等价于：

```mimi
func parse_and_double(s: string) -> Result<i64, string> {
    let n = match parse_int(s) {
        Ok(v) => v,
        Err(e) => return Err(e)
    };
    Ok(n * 2)
}
```

### 3.2 链式调用

```mimi
func process(config_path: string) -> Result<Config, string> {
    let data = read_file(config_path)?;    // 失败时提前返回
    let parsed = parse_json(data)?;        // 失败时提前返回
    let validated = validate(parsed)?;     // 失败时提前返回
    Ok(validated)
}
```

### 3.3 错误转换

```mimi
func process() -> Result<i32, string> {
    let n = risky_operation()?;   // 自动转换错误类型
    Ok(n)
}
```

---

## 4. Option 类型

### 4.1 T? 语法糖

`T?` 是 `Option<T>` 的语法糖：

```mimi
func find_user(id: i64) -> User? {
    // 返回 Some(user) 或 None
}

// 等价于
func find_user(id: i64) -> Option<User> {
    // ...
}
```

### 4.2 使用 Option

```mimi
match find_user(42) {
    Some(user) => println(user.name),
    None => println("not found")
}
```

### 4.3 ? 与 Option

在返回 Option 的函数中，`?` 传播 None：

```mimi
func get_first_name(id: i64) -> string? {
    let user = find_user(id)?;        // None 时提前返回 None
    let name = user.name?;            // None 时提前返回 None
    Some(name)
}
```

---

## 5. 错误传播链

### 5.1 多层错误传播

```mimi
func inner() -> Result<i32, string> {
    Err("inner error")
}

func middle() -> Result<i32, string> {
    let value = inner()?;   // 传播 "inner error"
    Ok(value)
}

func outer() -> Result<i32, string> {
    let value = middle()?;  // 传播 "inner error"
    Ok(value)
}

// outer() 返回 Err("inner error")
```

### 5.2 错误上下文

```mimi
func process() -> Result<i32, string> {
    let data = read_file("config.json")
        .map_err(|e| "failed to read config: " + e)?;
    let config = parse_json(data)
        .map_err(|e| "failed to parse config: " + e)?;
    Ok(config)
}
```

---

## 6. On Failure 补偿

### 6.1 基本用法

```mimi
func booking() -> Result<(), string> {
    let seat = reserve_seat()?;
    on failure { cancel_seat(seat); }

    let hotel = book_hotel()?;
    on failure { cancel_hotel(hotel); }

    let payment = charge()?;
    on failure { refund(payment); }

    Ok(())
}
```

执行流程：
1. `reserve_seat()` 成功，注册 `cancel_seat` 补偿
2. `book_hotel()` 成功，注册 `cancel_hotel` 补偿
3. `charge()` 失败 → 触发补偿栈
4. LIFO 顺序执行：`refund` → `cancel_hotel` → `cancel_seat`
5. 错误向上传播

### 6.2 补偿栈的作用域

```mimi
func process() -> Result<(), string> {
    let a = step1()?;
    on failure { undo1(a); }

    {
        let b = step2()?;
        on failure { undo2(b); }

        let c = step3()?;
        on failure { undo3(c); }
    }
    // 内层作用域退出时：undo3 → undo2 逆序执行

    Ok(())
}
```

### 6.3 补偿失败

补偿块本身失败时，错误累积为 `CompositeError`：

```mimi
func process() -> Result<(), string> {
    let resource = acquire()?;
    on failure {
        // 如果 release 失败，错误累积
        release(resource)?;
    }

    risky_operation()?;
    Ok(())
}
```

---

## 7. error 语句

### 7.1 不可恢复错误

```mimi
func validate(input: string) -> Result<(), string> {
    if input.is_empty() {
        error "input cannot be empty"
    }
    Ok(())
}
```

### 7.2 error 触发补偿

```mimi
func process() -> Result<(), string> {
    let resource = acquire()?;
    on failure { release(resource); }

    if something_wrong {
        error "validation failed"   // 触发 release 补偿
    }

    Ok(())
}
```

---

## 8. 显式忽略

### 8.1 drop 忽略 Result

```mimi
func main() -> i32 {
    let result = risky_operation();
    drop(result);   // 忽略 Result，不触发补偿

    0
}
```

### 8.2 忽略 Option

```mimi
func main() -> i32 {
    let optional = find_something();
    drop(optional);   // 忽略 Option

    0
}
```

---

## 9. 错误处理最佳实践

### 9.1 使用 ? 简化链式调用

```mimi
// ✅ 好：清晰简洁
func process() -> Result<Config, string> {
    let data = read(path)?;
    let json = parse(data)?;
    let config = validate(json)?;
    Ok(config)
}

// ❌ 差：嵌套过深
func process() -> Result<Config, string> {
    match read(path) {
        Ok(data) => match parse(data) {
            Ok(json) => match validate(json) {
                Ok(config) => Ok(config),
                Err(e) => Err(e)
            },
            Err(e) => Err(e)
        },
        Err(e) => Err(e)
    }
}
```

### 9.2 补偿操作保持简单

```mimi
// ✅ 好：补偿操作简单可靠
on failure { cancel_reservation(id); }

// ❌ 差：补偿操作本身可能失败
on failure {
    let refund = calculate_refund(amount)?;  // 可能失败
    process_refund(refund)?;                  // 可能失败
}
```

### 9.3 使用错误上下文

```mimi
// ✅ 好：有上下文信息
let data = read_file(path)
    .map_err(|e| "reading " + path + ": " + e)?;

// ❌ 差：无上下文
let data = read_file(path)?;
```
