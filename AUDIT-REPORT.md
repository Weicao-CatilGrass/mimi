# Mimi 编译器深度审计报告

> **项目**: Mimi v0.7.0 — 编译型系统编程语言，核心优势：类型安全的跨语言 FFI 编排
> **审计日期**: 2026-06-19（五轮评估整合）
> **代码规模**: Rust 31,639 行（源码）+ 17,770 行（测试）| C 1,277 行（运行时）| Mimi 2,090 行（标准库）
> **依赖**: 14 直接依赖 + 108 总依赖（Cargo.lock）

---

## 一、产品定位

> *Mimi 是一门完整的编译型系统编程语言，其核心优势在于类型安全的跨语言 FFI 编排——但所有系统编程能力（所有权、并发、模式匹配、泛型、合约验证）都完整可用，程序员可以选择用它写整个系统，也可以只用它做胶水。*

| 维度 | Python 胶水 | Mimi（目标） |
|------|-----------|-------------|
| 执行 | 解释器 + GIL | 编译原生码，无 GIL |
| FFI | CPython C API 包装层，有开销 | 直接 extern "C" 際开销 |
| 类型安全 | 运行时 duck typing | 编译期静态类型 |
| 性能 | 慢（解释执行） | 快（LLVM 优化） |
| 并发 | GIL 限制 | 真正并行 |
| 启动 | 慢（导入解释器） | 即时（已编译） |

### v1.0 发布标准

| 类别 | 必须就位 | 可以延后 |
|------|---------|---------|
| 系统编程基础 | 类型系统、所有权、match、泛型、trait | comptime 扩展 |
| 并发 | Actor 基本语义、spawn/await | 真正异步运行时 |
| FFI | 合约验证、C 头文件生成、多语言调用 Demo | 所有语言的预置绑定 |
| 标准库 | Result/Option、基本 IO、网络 HTTP | 完整序列化、加密 |

---

## 二、审计方法论

本报告基于五轮独立代码审计，逐项核实每个风险/缺口的**实际代码状态**（file:line 引用），而非推测。每条发现标注：

- **确认** — 代码证据完全支持
- **部分确认** — 核心问题存在但有缓解措施
- **已修复** — 上轮发现的问题已被代码修改解决

---

## 三、项目架构概览

```
Source (.mimi)
    │
    ▼
┌─────────┐     ┌─────────┐     ┌──────────┐
│  Lexer  │────▶│ Parser  │────▶│  Core    │
│ 1,149行 │     │ 2,738行 │     │ (类型检查)│
└─────────┘     └─────────┘     └────┬─────┘
                                     │
                    ┌────────────────┤
                    ▼                ▼
            ┌───────────┐    ┌───────────┐
            │  Interp   │    │  Codegen  │
            │ 5,691行   │    │ 8,200行   │
            │ (解释执行) │    │ (LLVM IR) │
            └─────┬─────┘    └─────┬─────┘
                  ▼                ▼
              Value<T>        Native Binary
                                     │
                              ┌──────┴──────┐
                              │ C Runtime   │
                              │ 1,277行      │
                              │ (malloc/map/ │
                              │  网络/线程池) │
                              └─────────────┘
```

辅助系统：
- **FFI 层** (`ffi/` + `interp/ffi_call.rs`) — **胶水语言核心**，跨语言调用边界
- **Verifier** (920行) — Z3 SMT 形式化验证
- **LSP** (1,089行) — 语言服务器
- **Formatter/Linter** — 代码格式化与静态分析
- **Diagnostic** — 错误诊断系统

---

## 四、风险总览

### 4.1 风险等级分布

| 等级 | 数量 | 条目 |
|------|------|------|
| **P0 — Critical** | 11 | F1-F6, G5, G10, B1, B2, B3 |
| **P1 — High** | 12 | F7-F9, G1-G2, G9, N2, N6, B4-B7 |
| **P2 — Medium** | 13 | G3-G4, G6, G8, N1, F10-F11, B8-B14 |
| **P3 — Low** | 7 | G7, N3-N5, B15-B17 |
| **已修复** | 11 | R3-R5, R7-R9, R12, R15-R18 等 |

---

## 五、P0 — FFI 层关键缺口（阻塞 v1.0）

### F1: 浮点 ABI 破损 — 静默数据损坏

**严重度**: P0
**位置**: `interp/ffi_call.rs:250-256`, `ffi/contract.rs:36`

**现状**: 所有 `f64` 参数通过 GP 寄存器（i64）传递，不使用 XMM 寄存器。

**证据**: `ffi_call.rs:88-94`:
> Float args are bit-cast to i64 in GP registers (NOT XMM0-7). C functions declared with `double` params will read wrong registers.

**后果**: 调用 SQLite `sqlite3_column_double`、OpenCV、BLAS 等库时产生**静默数据损坏**。

---

### F2: C 函数崩溃无恢复 — 进程不可恢复

**严重度**: P0
**位置**: `interp/ffi_call.rs:110-127`

**现状**: 无 SIGSEGV 信号处理。C 函数解引用空指针 → 进程直接死亡。

---

### F3: ensures 后置条件 result 绑定断裂 — 合约系统失效

**严重度**: P0
**位置**: `interp/ffi_call.rs:143-146`

**现状**: `ensures` 表达式中引用 `result` 变量会找不到绑定。`--verify-ffi` 模式的后置条件验证**完全不工作**。

---

### F4: SharedHandle RwLock guard 泄漏 — 每次 FFI 调用泄漏锁

**严重度**: P0
**位置**: `ffi/runtime.rs:149-165`

**现状**: `as_ptr()` 和 `as_mut_ptr()` 使用 `std::mem::forget(guard)` — 每次 `c_borrow`/`c_borrow_mut` 调用永久泄漏一个 RwLock guard。

---

### F5: FFI 类型映射不完整 — 胶水层半残

**严重度**: P0
**位置**: `ffi/contract.rs:141-177`

**现状**:

| 类型 | FFI 支持 | 说明 |
|------|---------|------|
| i32, i64, bool | ✅ | 值传递 |
| f64 | ⚠️ | 有 ABI 问题（见 F1） |
| string (borrow/transfer) | ✅ | CString 借用/转移 |
| raw pointer | ✅ | `*T`, `*mut T` |
| cap / c_shared / c_borrow | ✅ | 能力/共享/借用句柄 |
| **List** | ❌ | `Unsupported` |
| **Tuple** | ❌ | `Unsupported` |
| **Record** | ❌ | `Unsupported` |
| **Closure** | ❌ | 仅函数指针，无 env |
| **Actor** | ❌ | `Unsupported` |

---

### F6: FFI 内存契约不完整 — C 返回值泄漏

**严重度**: P0
**位置**: `interp/ffi_call.rs:449-464`

**现状**: C 函数返回字符串时，`to_string_lossy().into_owned()` 创建 Mimi 副本，但**不释放 C 侧分配**。同时 `to_string_lossy()` 静默替换非 UTF-8 字节。

---

### G5: Shared 引用计数缺失 — 语义分裂

**严重度**: P0
**位置**: `codegen/func.rs:652-659`, `codegen/block.rs:204-211`, `codegen/actors.rs:450-457`

| 层 | Shared 实现 | clone | drop |
|---|------------|-------|------|
| **interp** | `Arc<RwLock<Value>>` | `Arc::clone` 增引用计数 | 减计数，归零释放 |
| **codegen** | `alloca + store`（与 `let` 相同） | 独立栈副本 | 栈帧退出丢弃 |

---

### G10: 堆栈内存安全 — 编译产物系统性内存泄漏

**严重度**: P0
**位置**: `codegen/builtins/mod.rs:29-35`, `codegen/expr.rs` 多处 malloc

| 分配场景 | 对应 free | 状态 |
|---------|----------|------|
| spawn 结果 | `expr.rs:1524` | ✅ 配对 |
| list/map/string 构造 | 无 | ❌ 泄漏 |

---

## 六、P1 — 语言特性 FFI 角色评估

以下四个语言特性按 FFI 场景中的实际角色重排优先级。

### G2: 枚举 match tag — P0（从 P1 升级）

**FFI 角色**: C 函数返回 status code → 映射为枚举 → match 分派处理

**代码核实**: **部分确认** — 管线存在但断裂

| 组件 | 状态 | 位置 |
|------|------|------|
| `#[repr(C)]` 枚举注册为 `i32` | ✅ 已实现 | `registry.rs:322-326` |
| C header 生成 sequential integer tags | ✅ 已实现 | `c_header.rs:96-104` |
| match codegen 的 tag 比较 | ❌ **用 variant name 的 hash** | `expr.rs:907-909` |
| `from_int` / `int_to_enum` 转换 | ❌ **不存在** | 全局搜索零结果 |

**关键断裂**: codegen match 用 `name.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))` 做比较（`expr.rs:907-909`），C header 生成 `0, 1, 2, ...` sequential tags。**两者不兼容** — C 返回 `0` 不会匹配 variant name "Success" 的 hash。

**修复**: (1) 修复 match codegen 使用 ordinal index 而非 hash；(2) 添加 `from_int` builtin；(3) 确保 match discriminant 与 C header tag 一致。工期 1-2 周。

---

### G1: 闭包 — P1

**FFI 角色**: C callback → closure → Actor message

**代码核实**: **部分确认** — 基础设施存在，桥接缺失

| 组件 | 状态 | 位置 |
|------|------|------|
| `CallbackTable` 运行时 | ✅ 已实现 | `callback.rs:16-101` |
| `callback_trampoline` / `qsort_trampoline` | ✅ 已实现 | `callback.rs:87-117` |
| Mimi closure → C fn ptr 转换 | ❌ **不存在** | `interp/ffi_call.rs` 无 closure 处理 |
| codegen 闭包 struct `{fn_ptr, env}` | ❌ TODO | `expr.rs:1955-1957` |
| FFI 合约对 closure 参数的支持 | ❌ 映射为 `RawPtr` | `contract.rs:167-170` |

**修复**: (1) codegen 闭包 struct + env 打包（依赖 G5）；(2) trampoline 生成；(3) CallbackTable 集成。工期 2-4 周。

---

### G5: Shared — P1（从 P0 调整为特性级 P0，此处保持 FFI 视角 P1）

**FFI 角色**: 多个 Actor 共享 FFI 连接池（如 HTTP 连接池）

**代码核实**: **部分确认** — 核心机制可用，有泄漏问题

| 组件 | 状态 | 位置 |
|------|------|------|
| `SharedHandleTable` + retain/release | ✅ 已实现 | `runtime.rs:105-264` |
| `c_shared` 参数传递 | ✅ 已实现 | `ffi_call.rs:394-414` |
| codegen `c_shared` retain/release | ✅ 已实现 | `registry.rs:143-267` |
| handle 去重 | ❌ 每次 create 新 ID | `runtime.rs:203-208` |
| FFI 调用后 handle 清理 | ❌ 无 cleanup | `ffi_call.rs:394-414` |

**修复**: handle 去重 + 调用后 cleanup。工期 1-2 周。依赖 F4（guard 泄漏修复）。

---

### comptime — P2

**FFI 角色**: 编译期生成 FFI 绑定代码

**代码核实**: **部分确认** — 有 comptime 求值，无 C header 解析

| 组件 | 状态 | 位置 |
|------|------|------|
| 解释器 comptime 求值 | ✅ 已实现 | `interp/quote.rs` |
| C header → Mimi extern 生成 | ❌ **不存在** | `c_header.rs` 仅 Mimi→C 方向 |
| C header 解析器 | ❌ 不存在 | — |

**修复**: 需实现 C header 解析 + extern 声明生成。工期 3-4 周。v1.0 可延后。

---

## 七、特性依赖关系

```
Phase 1: FFI 可信基础 (前置条件)
├── G5: Shared RC ──────────────────┐
├── F4: guard 泄漏修复 ─────────────┤
├── F1: 浮点 ABI 修正 ──────────────┤
├── F3: ensures result 修复 ────────┤
├── F2: C 崩溃恢复 ────────────────┤
└── G10: 内存泄漏修复 ──────────────┘
                │
                ▼
Phase 2: 语言特性 FFI 就绪
├── G2: 枚举 match (P0) ── 需 hash→ordinal + from_int
├── G1: 闭包 (P1) ──────── 需闭包 struct (依赖 G5)
├── G5: Shared (P1) ────── 需 F4 修后 + handle 去重
└── comptime (P2) ──────── 独立路径
                │
                ▼
Phase 3: 工程化 + 绑定
├── G9: 跨文件模块
├── F9: Python binding
└── N6: ASan 启用
```

**关键约束**: 四个特性之间**不存在直接依赖**，但都依赖 Phase 1 的 FFI 基础修复。

---

## 八、P1 — 其他高优先级

### F7: extern ABI 无运行时校验

**位置**: `interp/ffi_call.rs:116` — 符号强转 `fn(i64×8)→i64`，无签名检查。

### F8: 跨语言回调仅脚手架

**位置**: `ffi/callback.rs` — CallbackTable 存在但无 Mimi closure 集成。

### N6: ASan/UBSan 测试全部禁用

**位置**: `tests/codegen_e2e.rs:1012-1308` — 9 个内存安全测试全部 `#[ignore]`。

### F9: 多语言绑定生成不存在

**位置**: 无实现。`emit-c-headers` 可生成 C 头文件，无其他语言绑定。

### G9: 跨文件模块 flatten

**位置**: `loader.rs:207-221` — `merge_all()` flatten 为单一 AST。E2E 测试框架不支持 `use`。

### N2: async 结果 await 侧截断为 i64

**位置**: `codegen/expr.rs:1511-1522` — await 始终 `build_load(i64)`，截断复合类型。

---

## 九、P2 — 中优先级

| 项 | 位置 | 状态 |
|----|------|------|
| G3: if 内 break/continue | `codegen/block.rs:190`, `func.rs:592-613` | func 级已覆盖，IR 测试通过 |
| G4: ? 运算符 E2E | `codegen/expr.rs:1535-1619` | 实现完整，E2E 测试绕过 |
| G6: Arena 降级 | `codegen/block.rs:217-239` | stacksave/stackrestore，功能等价 |
| G8: async pthreads | `codegen/func.rs:15-61` | 脱糖为 spawn+pthread，IR 测试通过 |
| N1: ring-buffer 溢出 | `runtime.c:701` | `pool_task_tail++` 无上限检查 |
| F10/F11: errno/UTF-8 | `ffi_call.rs:175-218,463` | ~35 errno 值，lossy 转换 |

---

## 十、P3 — 低优先级 / 设计如此

| 项 | 位置 | 状态 |
|----|------|------|
| G7: 借用检查不在 codegen | `core/mod.rs:109-273` | 设计如此 — core/ 已检查 |
| N3: 无结构化并发 | `codegen/expr.rs:1349-1463` | 胶水层不需要 |
| N4: E2E 框架不支持 `use` | `tests/mod.rs:1093-1095` | 测试框架限制 |
| N5: LSP 全量重解析 | `lsp.rs:146,152` | 非 bug，影响 UX |

---

---

## 十-B、未覆盖模块深度审计（2026-06-19 补充）

> **范围**: AUDIT-REPORT 前几轮未覆盖的 22 个文件，包括 `manifest.rs`、`lockfile.rs`、`safe_arith.rs`、`lint.rs`、`fmt.rs`、`error.rs`、`span.rs`、`diagnostic/`、`contracts.rs`（Mimi 合约系统）、`ast.rs`、`lexer.rs`、`interp/pattern.rs`、`interp/quote.rs`、`interp/pool.rs`、`loader.rs`、`codegen/builtins/io.rs` 等。

### P0 — 新增 Critical（3 个）

#### B1: Slice pattern rest-binding 静默忽略匹配失败

**严重度**: P0
**位置**: `interp/pattern.rs:110`

**现状**: `Pattern::Slice` 带 `rest_pat` 时，调用 `match_pattern_inner(rest_pat, &Value::List(remaining), bindings)` 但**丢弃返回值**。若 rest 模式不匹配（如 `[x, ..Rest(_, _)]` 需要构造函数），函数仍返回 `true`。

```rust
// line 110 — 返回值被忽略
self.match_pattern_inner(rest_pat, &Value::List(remaining), bindings);
// 随后无条件 `true`
```

**后果**: 任何带类型 rest 的 slice 模式会在错误数据上**静默成功**，运行时逻辑错误。

**修复**: 改为 `return self.match_pattern_inner(rest_pat, &Value::List(remaining), bindings);`

---

#### B2: `compile_read_file` 忽略 `fseek` 返回值 → 无界 malloc / 崩溃

**严重度**: P0
**位置**: `codegen/builtins/io.rs:481-502`

**现状**: `fseek(file, 0, SEEK_END)` 的 C 返回值（`int`）被丢弃（`.map_err(...)?` 只检查 LLVM IR 构建是否成功，不检查运行时 C 返回值）。若 `fseek` 失败，`ftell` 返回 `-1`（i64），随后用于 `malloc(size+1)` — 巨大或负数分配大小，导致 `malloc` 失败或 UB。

```rust
// line 481-486: fseek 的 C int 返回值未被检查
self.builder.build_call(fseek_fn, &[...], "fseek_call")
    .map_err(|e| CompileError::LlvmError(...))?;
// line 496-502: ftell 结果直接作为 malloc 大小
let file_size = self.builder.build_call(ftell_fn, &[...], "ftell_call")...;
```

**后果**: 编译产物的 `read_file` 内置函数在文件 I/O 错误时产生**内存损坏或崩溃**。

**修复**: 在 `fseek` 后比较其返回值；非零时返回错误或空字符串。

---

#### B3: `CompileError::code()` 错误码映射大面积错误

**严重度**: P0
**位置**: `error.rs:108-147`

**现状**: 多个 variant 映射到错误代码，与 `diagnostic/codes.rs` 及文档范围矛盾：

| Variant | 当前（错误） | 正确值 |
|---------|-------------|--------|
| `FieldNotFound` | `"E0703"` (line 116) | `"E0220"` |
| `ActorNotStruct` | `"E0703"` (line 113) | `"E0707"` |
| `NotStruct` | `"E0703"` (line 118) | `"E0707"` |
| `TypeMismatch` | `"E0712"` (line 117) | `"E0200"` |
| `WrongArgCount` | `"E0711"` (line 119) | `"E0210"` |
| `CapConsumed` | `"E0718"` (line 121) | `"E0304"` |
| `FfiWrapper` | `"E0710"` (line 143) | 与 `ExternNotDeclared` 碰撞 |
| `GenericsError` | `"E0720"` (line 136) | 与 `TurbofishArgCount` 碰撞 |

**后果**: 任何通过 `CompileError::code()` 路由的诊断显示**错误/误导性错误码**。LSP 的 `--explain`、错误码查找工具链完全不可用。

**修复**: 逐一核对 `diagnostic/codes.rs`，修正所有映射。

---

### P1 — 新增 High（4 个）

#### B4: Formatter 把 `)` 结尾行当成缩进增加触发器

**严重度**: P1
**位置**: `fmt.rs:46`

**现状（2026-06-19 核查）**: 当前代码 `trimmed.ends_with('{') || trimmed.ends_with('(') || trimmed.ends_with('[')` — **不含** `ends_with(')')`。审计报告撰写时记录的 `ends_with(')')` 问题在当前代码中已不存在。经核查，`starts_with(')')` 位于 line 35（减少缩进侧），使闭合 `)` 回到父级缩进，行为合理。**此项在当前代码中已不存在，无需修复。**

---

#### B5: Linter W001 对每个 `desc`/`rule` 都报警告

**严重度**: P1
**位置**: `lint.rs:33-47`

**现状**: `Item::Desc` 和 `Item::Rule` **无条件**发出 W001 警告。正常模式 `desc` + `func` 会被误报。W001 的语义是"standalone `desc`/`rule` 无后续实现"，但代码**不检查后续是否有 `func`**。

```rust
// lines 33-47: 不检查 "this is followed by a func/type"
Item::Desc(_text) => {
    diagnostics.push(Diagnostic::warning_code("W001", ...));
}
```

**后果**: 任何使用 `desc`/`rule` + `func` 的文件产生 **100% 误报**。

**修复**: 检查 `file.items` 中 desc/rule 的下一项是否为 `Func` 或 `Type`。

---

#### B6: F-string lexer 转义序列处理不完整

**严重度**: P1
**位置**: `lexer.rs:562-575`

**现状**: `scan_fstring` 处理 `\n`、`\t`、`\r` 时正确写入字面量，但 `\u{...}`、`\xNN` 等 Unicode/hex 转义**未被识别**。不识别的转义被静默透传（`line 572 s.push(c)` after advance），生成与预期不同的字符串值。

**后果**: F-string 中的 Unicode 转义静默产生错误字符串，难以追踪。

**修复**: 添加 `\u{...}` 和 `\xNN` 处理分支；未识别转义报 lexer 错误而非静默透传。

---

#### B7: Contract 语句全部报 `Span(0:0)` — 合约错误无法定位

**严重度**: P1
**位置**: `contracts.rs:45, 50`

**现状**: `Stmt::Requires(expr, Span::single(0, 0))` 和 `Stmt::Ensures(expr, Span::single(0, 0))` 硬编码 line 0:0。`parse_condition` 使用 `Lexer::new(text).tokenize()` 无行号上下文，即使修正 span 也会全部指向 line 1。

**后果**: 合约内类型错误/违反**永远指向文件顶部**，合约调试几乎不可能。

**修复**: 传入原始行号偏移给 `parse_condition`，使 span 指向合约所在行。

---

### P2 — 新增 Medium（7 个）

#### B8: `checked_div` / `checked_rem` 冗余零检查 + 溢出覆盖缺失

**严重度**: P2
**位置**: `safe_arith.rs:20-24, 28-34`

**现状**: 两个函数在调用 Rust 内置 `checked_div`/`checked_rem` 前显式检查 `b == 0 → None`。Rust 内置已处理除零（返回 `None`），冗余检查无害但**掩盖了 `i64::MIN % -1` 溢出场景** — Rust 的 `checked_rem` 对此返回 `Some(i64::MIN)`（非 `None`），而冗余 guard 使该溢出情况不被暴露。

**修复**: 移除冗余零检查，依赖内置行为；`MIN % -1` 溢出显式处理。

---

#### B9: lockfile 精确版本解析测试语义错误

**严重度**: P2
**位置**: `lockfile.rs:129-133`

**现状**: 测试 `resolve_version_exact` 断言 `"1.0.0"` 在 `["0.1.0", "0.2.0", "1.0.0"]` 中解析为 `"1.0.0"`。但 `semver::VersionReq::parse("1.0.0")` 解析为 `^1.0.0`（任意 `>=1.0.0 <2.0.0`）。若可用列表含 `"1.1.0"`，测试将失败（返回 `"1.1.0"`）。测试通过仅因可用列表不含更高匹配版本。

**修复**: 测试使用 `=1.0.0` 精确语法或扩展测试用例。

---

#### B10: `Span::contains` 多行 span 列检查缺失

**严重度**: P2
**位置**: `span.rs:45-56`

**现状**: `contains` 仅在 `line == self.end_line` 时检查 `col > self.end_col`。对于多行 span（如 `1:5-3:10`），line 2 col 20 会被报告为 `true`（在 span 内），尽管超出逻辑终点。缺少中间行和起始行的列边界检查。

**修复**: 添加 `line == self.start_line → col >= self.start_col` 以及中间行无条件包含逻辑。

---

#### B11: `interp/pool.rs` spawn 发送错误静默丢弃

**严重度**: P2
**位置**: `interp/pool.rs:30-32`

**现状**: `execute` 使用 `let _ = self.sender.send(...)`。若线程池 receiver 被丢弃（所有 worker panic），send 静默失败。生产环境中 spawn 操作**假成功**，任务永不执行。

**修复**: 将 `send` 错误向上传播或至少记录警告日志。

---

#### B12: Diagnostic format note 指示器列对齐错误

**严重度**: P2
**位置**: `diagnostic/format.rs:128-136`

**现状**: note 跨行时（`note.span.end_line != note.span.start_line`），指示器行使用 `" ".repeat(start_col) + note.message`，将 note 消息直接放在代码行下方而非对应 span 列下方。 gutter 宽度基于 `end_line` 位数而非实际 span 宽度，导致格式化错位。

**修复**: 使用 `start_col` 计算正确缩进 + 添加 `^^^^^` 下划线指示器。

---

#### B13: `interp/quote.rs` 双克隆冗余 RC bump

**严重度**: P2
**位置**: `interp/quote.rs:290`

**现状**: `Ok(*v.clone())` — `v` 是 `Box<Value>`。`v.clone()` 克隆 Box（增加 RC），`*v.clone()` 解引用为 `Value`，`Ok(...)` 再 move。属于**双重引用计数递增**。

```rust
// line 290
Ok(*v.clone())  // 应为 Ok(*v)
```

**修复**: 改为 `Ok(*v)`。

---

#### B14: `loader.rs` `merge_all` 导入与 item 重复 — 潜在 name collision

**严重度**: P2
**位置**: `loader.rs:207-221`

**现状**: `merge_all` 从每个模块收集 `all_imports` 放入合并后的 `File`。但 `File.imports` 代表待解析的 `use` 语句。合并后解释器看到 `use foo` 其中 `foo` 已是 item — 导入既是 item 又是未解析导入。可能导致**双重查找或 name collision**。

**修复**: 合并后清除已解析的导入，或将 `imports` 仅保留跨模块未满足的引用。

---

### P3 — 新增 Low（3 个）

#### B15: `Span::width()` 对多行 span 返回 0

**严重度**: P3
**位置**: `span.rs:59-65`

**现状**: 多行 span 的 `width()` 返回 0。任何依赖它的工具（下划线长度、caret 定位）对多行诊断输出不正确。

**修复**: 返回 `(end_line - start_line) * line_width + end_col - start_col` 或文档说明仅限单行。

---

#### B16: `manifest.rs` root 路径 `parent()` 循环无意义

**严重度**: P3
**位置**: `manifest.rs:52-53`

**现状**: `dir.parent().unwrap_or(&dir)` — 若 `dir` 是 root 路径（如 `/`），`parent()` 返回 `None`，`&dir`（`/`）被使用。随后 `dir.pop()` 对 `/` 返回 `false`（无法 pop 超过 root），多做一轮空循环。

**修复**: 对 root 路径提前 `return Ok(None)`。

---

#### B17: `interp/quote.rs:290` 性能：`*v.clone()` 多余引用计数递增

**严重度**: P3（注：与 B13 同一位置，此处作为性能问题单独记录）
**位置**: `interp/quote.rs:290`

**现状**: `Box<Value>` 的 `clone()` 递增 RC，随后立即解引用 move。在 comptime 求值热路径中，不必要的 RC 操作影响性能。

**修复**: 改为 `Ok(*v)`（同 B13 修复）。

---

### 修复状态（2026-06-19）

| 问题 | 状态 | 修复说明 |
|------|------|---------|
| B1 (slice pattern rest-binding) | ✅ 已修复 | `interp/pattern.rs:110` 添加 `return` 传播 rest 匹配结果 |
| B2 (fseek/ftell 无界 malloc) | ✅ 已修复 | `codegen/builtins/io.rs:481-530` 捕获 fseek 返回值，ftell 结果 clamp 到 0 |
| B3 (CompileError::code 映射) | ✅ 已修复 | `error.rs:108-148` 改用 `diagnostic/codes::*` 常量，消除全部错误映射 |
| B4 (formatter `)` 缩进) | ℹ️ 无需修复 | 当前代码 line 46 已不含 `ends_with(')')`，审计时记录的问题已不存在 |
| B5 (linter W001 误报) | ✅ 已修复 | `lint.rs:28-52` 新增 `is_followed_by_impl` 检查，desc/rule 后有 func/type 时不报警 |
| B6-B17（P2/P3） | ⏸️ 待修复 | 按路线图 Phase 3 排期处理 |

---

## 十一、历史风险项状态

### 已修复（11 项）

| # | 风险项 | 原等级 | 修复位置 |
|---|--------|--------|----------|
| R3 | LSP Content-Length DOS | Critical | `lsp.rs:6,45` — `MAX_CONTENT_LENGTH = 16MB` |
| R4 | Z3 缺失时 panic | Critical | `verifier.rs:40-43` — `catch_unwind` |
| R5 | 能力表全局状态无锁 | Critical | `runtime.c:536-559` — `cap_mutex` |
| R7 | calloc 整数溢出 | High | `runtime.c:44` — `SIZE_MAX / size` 检查 |
| R8 | Verifier 无超时 | High | `verifier.rs:9,48-50` — `5000ms` |
| R9 | Mutex 中毒未处理 | High | `pool.rs:18`, `runtime.rs:472` — `unwrap_or_else` |
| R10 | 模块导入路径遍历 | High | `loader.rs:137-144` — `..` 拒绝 |
| R12 | Verifier Box::leak | Medium | `verifier.rs` — 完全移除 |
| R15 | strcpy/strcat 无边界 | Medium | 调用点 `malloc(strlen()+1)` |
| R16 | str_replace 大小溢出 | Medium | `runtime.c:598-603` — 有符号算术 |
| R17 | mimi_try_exit 指针试探 | Low | `runtime.c:508-521` — 启发式移除 |

### 降级项（4 项）

| # | 风险项 | 原等级 | 当前 |
|---|--------|--------|------|
| R1 | FFI 签名类型混淆 | Critical | High |
| R2 | transmute 到函数指针 | Critical | Medium |
| R14 | LSP exit 跳过析构 | Low | Low |
| R18 | C 线程池全局状态 | Medium | Medium |

---

## 十二、FFI 层详细审计

### 12.1 类型映射能力矩阵

```
Mimi 类型         → C ABI 表示              → 状态
─────────────────────────────────────────────────────
i32, i64, bool    → int64_t 值传递          ✅ 可用
f64               → double (但走 GP 寄存器) ⚠️ ABI 错误
string (borrow)   → const char* 临时借用     ✅ 可用
string (transfer) → char* 所有权转移         ✅ 可用
*T, *mut T        → T* 原始指针             ✅ 可用
cap               → i64 能力句柄             ✅ 可用
c_shared T        → i64 共享句柄             ✅ 可用
c_borrow T        → T* 借用指针             ⚠️ guard 泄漏
List/Tuple/Record → Unsupported             ❌ 不可用
Closure           → 函数指针 (无 env)       ⚠️ 部分可用
Actor             → Unsupported             ❌ 不可用
```

### 12.2 内存所有权模型

```
方向          机制                    状态
──────────────────────────────────────────────
Mimi → C     StringBorrow (CString 临时)  ✅ 正确
Mimi → C     StringTransfer (into_raw)   ✅ 正确
C → Mimi     CStr::from_ptr + to_string  ⚠️ 泄漏 C 侧分配
Shared       SharedHandle + Arc<RwLock>  ⚠️ guard 泄漏
Cap          CapTable 注册/检查/消耗      ✅ 正确
```

### 12.3 FFI 测试覆盖

| 测试文件 | 测试数 | 覆盖范围 |
|---------|--------|---------|
| `ffi_safety.rs` | 17 | 类型拒绝/接受 |
| `ffi_passport_types.rs` | 8 | c_shared/c_borrow、cap |
| `ffi_verification.rs` | 7 | 合约生成、errno |
| `extern_calls.rs` | 4 | 符号未找到 |
| `extern_blocks.rs` | 5 | 解析、cap 参数 |
| `test-ffi-contracts.sh` | 9 | Z3 验证、运行时合约 |
| **总计** | **50** | |

**未覆盖**: 无端到端调用实际 C 函数的测试、无浮点 ABI 测试、无字符串返回所有权测试、无回调集成测试、无 ensures result 测试、无 FFI 模糊测试。

---

## 十三、统一根因分析

### 语言内部缺口

| 类型 | LLVM 表示 | 缺失部件 |
|------|----------|---------|
| **Shared** | `i8*` 裸指针 (`types.rs:50-53`) | 无 `{refcount, data}` 堆结构 |
| **Closure** | `i64` 裸整数 (`types.rs:93-96`) | 无 `{fn_ptr, env_ptr}` 结构体 |
| **Enum** | `{i32, i64}` 固定结构 | hash vs ordinal 不兼容 + 无 from_int |

### FFI 层缺口

| 缺口 | 根因 |
|------|------|
| F1 (浮点 ABI) | 调用约定硬编码为 GP 寄存器 |
| F2 (C 崩溃) | 无信号处理 |
| F3 (ensures) | eval 不支持 scope 注入 |
| F4 (guard 泄漏) | `mem::forget` 避免借用冲突 |
| F5 (类型映射) | 合约系统仅支持标量和指针 |
| F6 (内存契约) | C→Mimi 返回值无自动释放 |

---

## 十四、路线图

### Phase 0.5 — 诊断与运行时基础修复（P0-P1，1-2 天）

| 目标 | 工期 | 依赖 | 状态 |
|------|------|------|------|
| B3: `CompileError::code()` 映射修正 | 0.5 天 | 无 | ✅ 已修复 |
| B1: Slice pattern rest-binding 返回值修复 | 0.5 天 | 无 | ✅ 已修复 |
| B2: `fseek` 返回值检查 | 0.5 天 | 无 | ✅ 已修复 |
| B5: Linter W001 上下文感知 | 0.5 天 | 无 | ✅ 已修复 |
| B7: Contract span 行号传递 | 0.5 天 | 无 | ⏸️ 待修复 |
| B4: Formatter `)` 缩进 | — | — | ℹ️ 无需修复（当前代码已正确） |

### Phase 1 — FFI 可信基础（P0，3-5 天）

阻塞 v1.0。完成前不投入语言特性。

| 目标 | 工期 | 依赖 |
|------|------|------|
| G5: Shared 引用计数 | 0.5-1 天 | 无 |
| F4: RwLock guard 泄漏修复 | 0.5 天 | 无 |
| F1: 浮点 ABI 修正 | 1 天 | 无 |
| F3: ensures result 绑定修复 | 0.5 天 | 无 |
| F2: C 崩溃恢复 (SIGSEGV) | 1 天 | 无 |
| G10: 编译产物内存释放 | 1-2 天 | 依赖 G5 |

### Phase 2 — 语言特性 FFI 就绪（P0-P1，3-5 天）

| 目标 | 工期 | 依赖 |
|------|------|------|
| G2: 枚举 match tag (hash→ordinal + from_int) | 1-2 周 | 无 |
| G1: 闭包 env struct + trampoline | 2-4 周 | 依赖 G5 |
| Shared: handle 去重 + cleanup | 1-2 周 | 依赖 F4 |

### Phase 3 — 工程化（P1-P2，2-3 天）

| 目标 | 工期 | 依赖 |
|------|------|------|
| B4: Formatter `)` 缩进修复 | 0.5 天 | 无 |
| B6: F-string 转义序列补全 | 0.5 天 | 无 |
| B11: pool.rs send 错误传播 | 0.5 天 | 无 |
| B14: loader merge_all 导入清理 | 0.5 天 | 无 |
| G9: 跨文件模块 E2E | 1 天 | 无 |
| F9: Python binding generator | 1-2 天 | 无 |
| F10: errno 完整映射 | 0.5 天 | 无 |
| N6: 启用 ASan 测试 | 0.5 天 | 依赖 G10 |

### Phase 4 — 语言完善（P2-P3）

G3/G4 (测试覆盖)、N1 (ring-buffer)、G6/G8 (arena/async)、comptime (C header 解析) 等。

---

## 十五、压力集成测试建议

### 15.1 胶水层压力测试

建议新增 `e2e_glue_scenario.mimi`：
1. 定义 `Result<T,E>` 枚举跨 FFI 传递
2. 调用 C 库函数（strlen + 某个接受 double 的函数）
3. 通过 `c_shared` 传递 shared 状态给 C
4. match 枚举解构 C 返回值
5. 编译后运行 + valgrind 检测内存

### 15.2 多特性交叉测试

| 组合 | 风险 | 当前覆盖 |
|------|------|---------|
| enum match + ? | match 解构 Result 后 ? 传播 | 0 |
| FFI + shared + enum | C 函数接收/返回 shared 枚举值 | 0 |
| 闭包 + shared | 闭包捕获 shared 变量 → env 中引用计数 | 0 |
| shared + spawn | Actor 内部 shared 状态 + await 返回 | 0 |

---

## 十六、附录：关键文件索引

| 模块 | 关键文件 | 行数 |
|------|---------|------|
| 解析器 | `src/parser/{mod,parse_expr,parse_stmt,parse_type}.rs` | 2,738 |
| 类型检查 | `src/core/{mod,check_stmt,infer_expr}.rs` | 3,973 |
| 解释器 | `src/interp/{mod,eval,call,builtins,value}.rs` | 5,691 |
| 模式匹配 | `src/interp/pattern.rs` | 116 |
| 引用/QuasiQuote | `src/interp/quote.rs` | ~300 |
| 线程池 | `src/interp/pool.rs` | ~50 |
| 代码生成 | `src/codegen/{mod,expr,func,block,types,registry}.rs` | ~8,200 |
| 内置函数 | `src/codegen/builtins/{mod,io,string,list,map,json,network,time_env}.rs` | ~2,500 |
| **FFI** | **`src/ffi/{contract,runtime,callback,c_header}.rs` + `src/interp/ffi_call.rs`** | **~1,890** |
| 验证器 | `src/verifier.rs` | 1,153 |
| LSP | `src/lsp.rs` | 1,089 |
| C 运行时 | `src/runtime/mimi_runtime.{c,h}` | 1,277+122 |
| 测试 | `src/tests/` (66 文件) | 17,770 |
| FFI 文档 | `docs/ffi-glue.md`, `docs/ffi-ownership-abi.md` | 944 |
| 诊断/错误 | `src/{error,span,lint,fmt,contracts,ast,lexer,loader,manifest,lockfile,safe_arith}.rs` + `src/diagnostic/` | ~5,000 |

---

*本报告基于 2026-06-19 的代码状态（六轮评估整合）。Mimi 是完整的系统语言，FFI 是杀手级应用场景。所有语言特性服务于"让跨语言编排更安全、更可验证"。如语言版本升级，请同步修订。*
