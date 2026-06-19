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
| **P0 — Critical** | 8 | F1 (浮点 ABI), F2 (C 崩溃恢复), F3 (ensures 断裂), F4 (guard 泄漏), F5 (类型映射), F6 (内存契约), G5 (Shared RC), G10 (内存泄漏) |
| **P1 — High** | 8 | F7 (ABI 校验), F8 (跨语言回调), G2 (枚举 match), N6 (ASan 禁用), F9 (绑定生成), G9 (跨文件模块), G1 (闭包 env), N2 (async 截断) |
| **P2 — Medium** | 6 | G3 (if break/continue), G4 (? 运算符), G6 (arena), G8 (async), N1 (ring-buffer), F10 (errno), F11 (UTF-8) |
| **P3 — Low** | 4 | G7 (借用检查, 设计如此), N3 (结构化并发), N4 (E2E 框架), N5 (LSP 性能) |
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
| 代码生成 | `src/codegen/{mod,expr,func,block,types,registry}.rs` | ~8,200 |
| **FFI** | **`src/ffi/{contract,runtime,callback,c_header}.rs` + `src/interp/ffi_call.rs`** | **~1,890** |
| 验证器 | `src/verifier.rs` | 1,153 |
| LSP | `src/lsp.rs` | 1,089 |
| C 运行时 | `src/runtime/mimi_runtime.{c,h}` | 1,277+122 |
| 测试 | `src/tests/` (66 文件) | 17,770 |
| FFI 文档 | `docs/ffi-glue.md`, `docs/ffi-ownership-abi.md` | 944 |

---

*本报告基于 2026-06-19 的代码状态。Mimi 是完整的系统语言，FFI 是杀手级应用场景。所有语言特性服务于"让跨语言编排更安全、更可验证"。如语言版本升级，请同步修订。*
