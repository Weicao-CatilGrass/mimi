# 10 - FFI 与跨语言

---

## 1. 概述

Mimi 支持安全的跨语言集成，核心机制是 **cap 线性能力** 作为权限凭证。

| 方向 | 机制 |
|------|------|
| Mimi 调用外部 | `extern "C"` 块 + cap 授权 |
| 外部调用 Mimi | 编译为 C 动态库 + 自动生成绑定 |

---

## 2. extern "C" 声明

### 2.1 基本语法

```mimi
extern "C" {
    fn function_name(param: Type, ...) -> ReturnType;
}
```

### 2.2 带 cap 授权

```mimi
cap FileReadCap;

extern "C" {
    fn read_file(path: string, cap @fh: FileReadCap) -> string;
    fn write_file(path: string, data: string, cap @fh: FileWriteCap) -> Result<(), string>;
}
```

- `cap @fh: Type` — 移动语义的 cap（默认）
- `&fh: Type` — 借用语义的 cap

### 2.3 支持的参数类型

| Mimi 类型 | C 类型 |
|-----------|--------|
| `i32` | `int32_t` |
| `i64` | `int64_t` |
| `f64` | `double` |
| `bool` | `bool` |
| `string` | `const char*` |
| `cap` | 不透明句柄 |

---

## 3. 使用示例

### 3.1 调用 C 库

```mimi
cap SQLiteCap;

extern "C" {
    fn sqlite3_open(path: string, cap @db: SQLiteCap) -> Result<i64, string>;
    fn sqlite3_exec(db: i64, query: string, cap @db: SQLiteCap) -> Result<(), string>;
    fn sqlite3_close(db: i64, cap @db: SQLiteCap) -> Result<(), string>;
}

func init_database(path: string, cap: SQLiteCap) -> Result<i64, string> {
    let db = sqlite3_open(path, cap)?;
    Ok(db)
}

func query(db: i64, sql: string, cap: SQLiteCap) -> Result<(), string> {
    sqlite3_exec(db, sql, cap)?;
    Ok(())
}

func close_database(db: i64, cap: SQLiteCap) -> Result<(), string> {
    sqlite3_close(db, cap)?;
    Ok(())
}
```

### 3.2 并发调用外部服务

```mimi
cap NetworkCap;

extern "C" {
    fn http_get(url: string, cap @nc: NetworkCap) -> Result<string, string>;
    fn http_post(url: string, body: string, cap @nc: NetworkCap) -> Result<string, string>;
}

func sync_data(user_id: u64, net_cap: NetworkCap) -> Result<(), string> {
    parasteps "同步用户数据" {
        let profile = spawn http_get("api/users/" + to_string(user_id), net_cap);
        let orders = spawn http_get("api/orders/" + to_string(user_id), net_cap);
        let p = await profile;
        let o = await orders;
        process_profile(p)?;
        process_orders(o)?;
    }!
    Ok(())
}
```

### 3.3 带补偿的 FFI 调用

```mimi
cap FileWriteCap;

extern "C" {
    fn create_file(path: string, cap @fh: FileWriteCap) -> Result<i64, string>;
    fn write_fd(fd: i64, data: string, cap @fh: FileWriteCap) -> Result<(), string>;
    fn delete_file(path: string, cap @fh: FileWriteCap) -> Result<(), string>;
}

func safe_write(path: string, data: string, cap: FileWriteCap) -> Result<(), string> {
    let fd = create_file(path, cap)?;
    on failure {
        delete_file(path, cap);   // 补偿：清理文件
    }

    write_fd(fd, data, cap)?;
    Ok(())
}
```

---

## 4. Cap 与 FFI 安全

### 4.1 权限凭证模式

```mimi
cap FileReadCap;

extern "C" {
    fn read_file(path: string, cap @fh: FileReadCap) -> string;
}

// 没有 FileReadCap 就无法调用 read_file
func main() {
    // ❌ 编译错误：缺少 cap
    // let data = read_file("secret.txt");

    // ✅ 持有 cap 才能调用
    let data = read_file("secret.txt", my_file_read_cap);
}
```

### 4.2 Cap 组合

```mimi
cap FileReadCap;
cap FileWriteCap;
cap FullFileAccess = FileReadCap + FileWriteCap;

func full_task(cap: FullFileAccess) {
    let (read, write) = cap.split();
    let data = read_file("input.txt", read);
    write_file("output.txt", data, write);
}
```

---

## 5. Mimi 作为库

### 5.1 编译为 C 动态库

```bash
mimi build --emit-c-lib src/lib.mimi
```

生成产物：
- `lib.so` / `lib.dylib` / `lib.dll` — 动态库
- `lib.h` — C 头文件
- `lib.meta.json` — 元数据（desc/rule/requires/ensures）

### 5.2 C 头文件示例

```c
// 自动生成的 lib.h
#ifndef LIB_H
#define LIB_H

#include <stdint.h>

// 不透明 cap 句柄
typedef void* FileReadCapHandle;

// 函数声明
int32_t process_order(uint64_t order_id, FileReadCapHandle cap);
const char* get_status(uint64_t order_id);

#endif
```

### 5.3 元数据文件示例

```json
{
  "functions": {
    "process_order": {
      "desc": "处理订单：验证、扣款、发货",
      "rule": "订单必须幂等",
      "requires": "order.status == New",
      "ensures": "order.status == Paid",
      "params": [
        {"name": "order_id", "type": "u64"},
        {"name": "cap", "type": "FileReadCap"}
      ],
      "return": "i32"
    }
  }
}
```

---

## 6. 跨语言调用

### 6.1 Python 调用 Mimi

```python
# Python 端
import mimilib

# 获取能力令牌
cap = mimilib.FileReadCap()

# 调用 Mimi 函数
result = mimilib.process_order(order_id, cap)
print(result)
```

### 6.2 Rust 调用 Mimi

```rust
// Rust 端
use mimilib;

fn main() {
    let cap = unsafe { mimilib::FileReadCap::new() };
    let result = unsafe { mimilib::process_order(order_id, cap) };
    println!("{}", result);
}
```

### 6.3 Swift 调用 Mimi

```swift
// Swift 端
import MimiLib

let cap = FileReadCap()
let result = process_order(orderId: orderId, cap: cap)
print(result)
```

---

## 7. 双向胶水架构

```
┌─────────────────────────────────────────────────┐
│                  Mimi 核心                       │
│  ┌─────────────┐  ┌─────────────┐               │
│  │  cap 权限   │  │  契约意图   │               │
│  │  追踪       │  │  元数据     │               │
│  └──────┬──────┘  └──────┬──────┘               │
│         │                │                      │
│         ▼                ▼                      │
│  ┌─────────────────────────────────┐            │
│  │     C ABI 动态库               │            │
│  │  ┌───────┐  ┌───────┐          │            │
│  │  │ .so   │  │ .h    │          │            │
│  │  └───────┘  └───────┘          │            │
│  └─────────────────────────────────┘            │
└─────────────────────────────────────────────────┘
         │                │
         ▼                ▼
    ┌─────────┐     ┌─────────┐
    │ Python  │     │  Rust   │
    │ Swift   │     │  C++    │
    │ Kotlin  │     │  Go     │
    └─────────┘     └─────────┘
```

---

## 8. 安全性保证

| 保证 | 机制 |
|------|------|
| 权限静态检查 | cap 编译期追踪 |
| 无隐式权限获取 | cap 必须显式传递 |
| 补偿自动化 | on failure LIFO 执行 |
| 意图可追溯 | desc/rule/requires/ensures 元数据 |
| AI 可审计 | --strict 模式强制审查 |

---

## 9. 当前局限

| 局限 | 状态 |
|------|------|
| `unsafe` 块 | ⏳ 规划中 |
| FFI 类型转换自动化 | ⏳ 规划中 |
| WASM 编译目标 | ⏳ 规划中 |
| 自动绑定生成器 | ⏳ 规划中 |
