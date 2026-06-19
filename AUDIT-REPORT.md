# Mimi 编译器深度审计报告

> **项目**: Mimi v0.7.0 — 编译型胶水语言
> **审计日期**: 2026-06-19（四轮评估整合）
> **代码规模**: Rust 31,639 行（源码）+ 17,770 行（测试）| C 1,277 行（运行时）| Mimi 2,090 行（标准库）
> **依赖**: 14 直接依赖 + 108 总依赖（Cargo.lock）

---

## 一、产品定位与审计视角

Mimi 的核心定位是**编译型胶水语言**——替代 Python 在高性能场景下的胶水角色：

| 维度 | Python 胶水 | Mimi 胶水（目标） |
|------|-----------|-----------------|
| 执行 | 解释器 + GIL | 编译原生码，无 GIL |
| FFI | CPython C API 包装层，有开销 | 直接 extern "C" 零开销 |
| 类型安全 | 运行时 duck typing | 编译期静态类型 |
| 性能 | 慢（解释执行） | 快（LLVM 优化） |
| 并发 | GIL 限制 | 真正并行 |
| 启动 | 慢（导入解释器） | 即时（已编译） |
| 适用场景 | 快速粘合、原型 | 高性能粘合、生产级胶水 |

**审计视角**: 本报告以**胶水语言核心能力**为优先级基准，而非语言内部特性完整性。FFI 层是产品的生命线。

---

## 二、审计方法论

本报告基于四轮独立代码审计，逐项核实每个风险/缺口的**实际代码状态**（file:line 引用），而非推测。每条发现标注：

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

### 4.1 胶水语言视角优先级分布

```
                    Impact
              Low   Med   High  Critical
Occurrence
  Certain     -     -      -      F1,F2,F3,F4,F5,F6,G5,G10
  Likely      R14   N1     R1     -
  Possible    R17   G1,G2  F7,F8  -
  Rare        G9    G6,G8  G3,G4  -
  By Design   G7    -      -      -
```

### 4.2 风险等级分布（胶水语言视角）

| 等级 | 数量 | 条目 |
|------|------|------|
| **P0 — Critical** | 8 | F1 (浮点 ABI), F2 (C 崩溃恢复), F3 (ensures 断裂), F4 (guard 泄漏), F5 (类型映射), F6 (内存契约), G5 (Shared RC), G10 (内存泄漏) |
| **P1 — High** | 6 | F7 (ABI 校验), F8 (跨语言回调), G2 (枚举 match), N6 (ASan 禁用), F9 (绑定生成), G9 (跨文件模块) |
| **P2 — Medium** | 8 | G1 (闭包 env), N2 (async 截断), G3 (if break/continue), G4 (? 运算符), G6 (arena), G8 (async), N1 (ring-buffer), F10 (errno), F11 (UTF-8) |
| **P3 — Low** | 2 | G7 (借用检查, 设计如此), N3 (结构化并发), N4 (E2E 框架), N5 (LSP 性能) |
| **已修复** | 11 | R3-R5, R7-R9, R12, R15-R18 等 |

---

## 五、P0 — FFI 层关键缺口（阻塞胶水语言定位）

### F1: 浮点 ABI 破损 — 静默数据损坏

**严重度**: P0
**位置**: `interp/ffi_call.rs:250-256`, `ffi/contract.rs:36`

**现状**: 所有 `f64` 参数通过 GP 寄存器（i64）传递，不使用 XMM 寄存器。

**证据**: `ffi_call.rs:88-94` 安全注释明确承认：
> Float args are bit-cast to i64 in GP registers (NOT XMM0-7). C functions declared with `double` params will read wrong registers.

**后果**: 任何编译为正常 C ABI 的函数（`double` 参数在 XMM0）都会读到错误寄存器。调用 SQLite `sqlite3_column_double`、OpenCV、BLAS 等库时产生**静默数据损坏**。

**修复路径**: 使用 libffi CIF 描述符做正确的 ABI 编码，或根据参数类型选择 GP/XMM 寄存器。

---

### F2: C 函数崩溃无恢复 — 进程不可恢复

**严重度**: P0
**位置**: `interp/ffi_call.rs:110-127`

**现状**: 无 SIGSEGV 信号处理。C 函数解引用空指针 → 进程直接死亡。

**后果**: 胶水层调用的任何 C 函数崩溃都会导致整个 Mimi 进程不可恢复地终止。无法区分"C 返回错误"和"C 崩溃了"。这直接违背了胶水语言"安全桥接多种语言"的核心价值主张。

**修复路径**: 安装 SIGSEGV 信号处理（转为 Mimi `Err`），或使用 `fork`/子进程隔离 C 调用。

---

### F3: ensures 后置条件 result 绑定断裂 — 合约系统失效

**严重度**: P0
**位置**: `interp/ffi_call.rs:143-146`

**现状**: 注释写明 *"The eval_expr method doesn't support scope binding directly"*。`ensures` 表达式中引用 `result` 变量会找不到绑定。

**后果**: FFI 合约系统的后置条件验证**完全不工作**。`--verify-ffi` 模式下，`ensures: result > 0` 会失败或引用无关变量。Mimi 胶水层的"类型安全 + 合约验证"卖点形同虚设。

---

### F4: SharedHandle RwLock guard 泄漏 — 每次 FFI 调用泄漏锁

**严重度**: P0
**位置**: `ffi/runtime.rs:149-165`

**现状**: `as_ptr()` 和 `as_mut_ptr()` 使用 `std::mem::forget(guard)` — 每次 `c_borrow`/`c_borrow_mut` 调用永久泄漏一个 RwLock guard。

**后果**: 每次通过 FFI 读写 shared 数据都会泄漏一个 guard 并持有读/写锁。累积后导致内存增长和潜在死锁。与 G5（Shared RC）直接相关——即使修复了引用计数，guard 泄漏仍会导致 shared 数据的 FFI 传递有内存问题。

---

### F5: FFI 类型映射不完整 — 胶水层半残

**严重度**: P0
**位置**: `ffi/contract.rs:141-177`

**现状**:

| 类型 | FFI 支持 | 说明 |
|------|---------|------|
| i32, i64, bool | ✅ | 值传递 |
| f64 | ⚠️ | 有 ABI 问题（见 F1） |
| string (borrow) | ✅ | `CString` 临时借用 |
| string (transfer) | ✅ | `CString::into_raw` 所有权转移 |
| raw pointer | ✅ | `*T`, `*mut T` |
| cap | ✅ | 能力句柄 |
| c_shared / c_borrow | ✅ | 共享/借用边界句柄 |
| **List** | ❌ | `Unsupported` |
| **Tuple** | ❌ | `Unsupported` |
| **Record** | ❌ | `Unsupported` |
| **Closure** | ❌ | 仅作为函数指针传递，无 env |
| **Actor** | ❌ | `Unsupported` |

**后果**: List/Record 是胶水层最常用的数据交换格式（JSON 数组→List，C struct→Record）。不支持意味着 Mimi 无法直接与 C 库交换复杂数据结构。

---

### F6: FFI 内存契约不完整 — C 返回值泄漏

**严重度**: P0
**位置**: `interp/ffi_call.rs:449-464`

**现状**: C 函数返回字符串时，`CStr::from_ptr` + `to_string_lossy().into_owned()` 创建 Mimi 拥有的新字符串，但**不释放 C 侧分配**。

**后果**: 如果 C 函数返回 `malloc` 分配的字符串，Mimi 拿到副本后 C 侧原指针无人释放。每次调用泄漏一次。

**同时**: `to_string_lossy()` 静默替换非 UTF-8 字节为 U+FFFD（`ffi_call.rs:463`），C 字符串中的非 ASCII 数据会丢失。

---

### G5: Shared 引用计数缺失 — 语义分裂

**严重度**: P0（从 P1 升级）
**位置**: `codegen/func.rs:652-659`, `codegen/block.rs:204-211`, `codegen/actors.rs:450-457`

**现状**:

| 层 | Shared 实现 | clone 行为 | drop 行为 |
|---|------------|-----------|----------|
| **interp** | `Value::Shared(Arc<RwLock<Value>>)` (`interp/value.rs:95`) | `Arc::clone` 增引用计数 | Arc drop 减计数，归零释放 |
| **codegen** | `alloca + store` — 与普通 `let` 完全相同 | 独立栈副本，无引用计数 | 栈帧退出直接丢弃 |

**后果**: 胶水层跨语言传递 shared 数据时，编译后的行为与解释器不同。interp 中 `shared x = y` 创建共享引用，codegen 中创建独立副本。并发程序编译后产生数据竞争。

---

### G10: 堆栈内存安全 — 编译产物系统性内存泄漏

**严重度**: P0（从 P1 升级）
**位置**: `codegen/builtins/mod.rs:29-35`, `codegen/expr.rs` 多处 malloc

**现状**: 编译输出使用裸 `malloc`/`free`，无引用计数或 GC。

| 分配场景 | malloc 位置 | 对应 free | 状态 |
|---------|------------|----------|------|
| spawn 结果传递 | `expr.rs:1376` | `expr.rs:1524` | ✅ 配对 |
| list 构造 | `builtins/list.rs:31` | 无 | ❌ 泄漏 |
| map 构造 | `builtins/map.rs:209` | 无 | ❌ 泄漏 |
| 字符串插值 | `expr.rs:2310` | 无 | ❌ 泄漏 |
| string 操作 | `builtins/string.rs` 多处 | 无 | ❌ 泄漏 |

**后果**: 每次 FFI 调用都可能触发 list/map/string 分配。编译后的胶水层在运行期间持续泄漏。长时间运行的服务会 OOM。

---

## 六、P1 — 高优先级

### F7: extern ABI 无运行时校验

**严重度**: P1
**位置**: `interp/ffi_call.rs:116`

**现状**: 符号强转为 `fn(i64×8)→i64`，无运行时签名检查。如果用户声明 `extern "C" fn foo(x: i64) -> i64` 但实际 C 函数签名不匹配，静默数据损坏。

---

### F8: 跨语言回调仅脚手架

**严重度**: P1
**位置**: `ffi/callback.rs` (146行)

**现状**: `CallbackTable` 和 trampoline 存在，但：
- 无 Mimi 闭包→C 函数指针的转换机制
- trampoline 仅支持 i64 参数，不支持 string/list
- `unsafe impl Send + Sync` 在捕获值的句柄上（`callback.rs:27-28`）
- `qsort_trampoline` 解引用原始指针无验证（`callback.rs:110`）

---

### G2: 枚举构造器 match 不完整

**严重度**: P1（从 P1 保持，胶水级不变）
**位置**: `codegen/expr.rs:965-977`, `codegen/registry.rs:322-331`

**现状**: 枚举类型注册为 `{i32 tag, i64 payload}` 结构体（已实现）。但 match 的 `Constructor` 分支只绑定原始值，不提取 tag 做比较。E2E 测试用 if/else 替代。

**胶水层影响**: ADT 是胶水层核心数据交换格式（C enum↔Mimi enum，Result 类型跨语言传递）。match 不完整直接影响数据解构。

---

### N6: ASan/UBSan 测试全部禁用

**严重度**: P1
**位置**: `tests/codegen_e2e.rs:1012-1308`

**现状**: 9 个内存安全测试全部 `#[ignore]`。`run-ci-matrix.sh` 显式传 `--ignored` 运行，但无自动化 CI 配置。日常 `cargo test` 跳过所有内存安全验证。

---

### F9: 多语言绑定生成不存在

**严重度**: P1
**位置**: 无实现

**现状**: 无 Python/TS/Swift binding generator。文档（`docs/ffi-glue.md`）描述了分层愿景但无代码。`emit-c-headers` 命令可生成 C 头文件，但无其他语言的绑定。

---

### G9: 跨文件模块 flatten

**严重度**: P1（从 P3 升级，胶水级需要）
**位置**: `loader.rs:207-221`, `main.rs:1086-1094`

**现状**: `merge_all()` 将所有模块 flatten 为单一 AST。编译命令支持，E2E 测试框架不支持 `use` 导入。胶水逻辑需要多文件组织。

---

## 七、P2 — 中优先级

### G1: 闭包 env 结构体缺失

**严重度**: P2（从 P1 降级，胶水级不需要闭包捕获）
**位置**: `codegen/expr.rs:1955-1957`

**现状**: 自由变量收集机制已实现，但返回值是裸函数指针，缺少 `{fn_ptr, env_ptr}` 结构体。胶水语言传函数指针即可。

---

### N2: async 结果 await 侧截断为 i64

**严重度**: P2（从 P1 降级，胶水层不需要 async）
**位置**: `codegen/expr.rs:1511-1522`

**现状**: await 侧始终 cast 为 `i64*` + `build_load(i64)`，截断非 i64 结果。胶水层 spawn 传递简单值即可。

---

### G3: if 内 break/continue

**严重度**: P2
**位置**: `codegen/block.rs:190`, `codegen/func.rs:592-613`

**现状**: `compile_func` 级别已覆盖常见路径。IR 测试 3 个通过。

---

### G4: ? 运算符 E2E 路径

**严重度**: P2
**位置**: `codegen/expr.rs:1535-1619`

**现状**: `compile_try_expr` 完整实现。E2E 测试用 if/else 绕过，无 `?` 直接测试。

---

### G6: Arena 分配器降级

**严重度**: P2（胶水级 P3）
**位置**: `codegen/block.rs:217-239,283-318`

**现状**: Arena 编译为 `llvm.stacksave`/`stackrestore`。功能等价。

---

### G8: async/await pthreads 模拟

**严重度**: P2（胶水级 P3，宿主语言处理异步）
**位置**: `codegen/func.rs:15-61`, `codegen/expr.rs:1349-1533`

---

### N1: C 线程池 ring-buffer 溢出

**严重度**: Medium
**位置**: `runtime.c:701`

---

### F10/F11: errno 映射不全 / UTF-8 lossy

**严重度**: P2
**位置**: `ffi_call.rs:175-218` (errno ~35值), `ffi_call.rs:463` (lossy)

---

## 八、P3 — 低优先级 / 设计如此

| 项 | 位置 | 状态 |
|----|------|------|
| G7: 借用检查不在 codegen | `core/mod.rs:109-273` | 设计如此 — core/ 已检查，codegen 信任输入 |
| N3: 无结构化并发 | `codegen/expr.rs:1349-1463` | 胶水层不需要 |
| N4: E2E 测试框架不支持 `use` | `tests/mod.rs:1093-1095` | 测试框架限制 |
| N5: LSP 全量重解析 | `lsp.rs:146,152` | 非 bug，影响 UX |

---

## 九、历史风险项状态（已修复/缓解）

以下风险在审计中被发现后已修复或显著缓解：

| # | 风险项 | 原等级 | 当前 | 修复位置 |
|---|--------|--------|------|----------|
| R3 | LSP Content-Length DOS | Critical | **已修复** | `lsp.rs:6,45` — `MAX_CONTENT_LENGTH = 16MB` |
| R4 | Z3 缺失时 panic | Critical | **已修复** | `verifier.rs:40-43` — `catch_unwind` + 优雅错误 |
| R5 | 能力表全局状态无锁 | Critical | **已修复** | `runtime.c:536-559` — `pthread_mutex_t cap_mutex` |
| R7 | calloc 整数溢出 | High | **已修复** | `runtime.c:44` — `count > SIZE_MAX / size` 检查 |
| R8 | Verifier 无超时 | High | **已修复** | `verifier.rs:9,48-50` — `DEFAULT_TIMEOUT_MS = 5000` |
| R9 | Mutex 中毒未处理 | High | **已修复** | `pool.rs:18`, `runtime.rs:472` — `unwrap_or_else` |
| R10 | 模块导入路径遍历 | High | **已修复** | `loader.rs:137-144` — segment 含 `..`/`/`/`\` 拒绝 |
| R12 | Verifier Box::leak 泄漏 | Medium | **已修复** | `verifier.rs` — `Box::leak` 完全移除 |
| R15 | strcpy/strcat 无边界 | Medium | **已缓解** | 所有调用点 `malloc(strlen()+1)` |
| R16 | str_replace 大小溢出 | Medium | **已修复** | `runtime.c:598-603` — 有符号 `int64_t delta` 算术 |
| R17 | mimi_try_exit 指针试探 | Low | **已修复** | `runtime.c:508-521` — 启发式移除，仅打印数值 |

**降级项**:

| # | 风险项 | 原等级 | 当前 | 原因 |
|---|--------|--------|------|------|
| R1 | FFI 签名类型混淆 | Critical | **High** | 安全文档完善 + FfiContract 类型系统，但无 ABI 运行时校验 |
| R2 | transmute 到函数指针 | Critical | **Medium** | null/align 检查 + `transmute_copy` + `debug_assert` |
| R14 | LSP exit 跳过析构 | Low | Low | 加了 flush，`process::exit(0)` 仍存在 |
| R18 | C 线程池全局状态 | Medium | Medium | mutex 覆盖所有路径；ring-buffer 溢出未检查 |

---

## 十、FFI 层详细审计

### 10.1 类型映射能力矩阵

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
c_borrow_mut T    → T* 可变借用             ⚠️ guard 泄漏
List              → Unsupported             ❌ 不可用
Tuple             → Unsupported             ❌ 不可用
Record            → Unsupported             ❌ 不可用
Closure           → 函数指针 (无 env)       ⚠️ 部分可用
Actor             → Unsupported             ❌ 不可用
```

### 10.2 内存所有权模型

```
方向          机制                    状态
──────────────────────────────────────────────
Mimi → C     StringBorrow (CString 临时)  ✅ 正确
Mimi → C     StringTransfer (into_raw)   ✅ 正确
C → Mimi     CStr::from_ptr + to_string  ⚠️ 泄漏 C 侧分配
C → Mimi     CStr::from_ptr + lossy      ⚠️ UTF-8 数据丢失
Shared       SharedHandle + Arc<RwLock>  ⚠️ guard 泄漏
Cap          CapTable 注册/检查/消耗      ✅ 正确
```

### 10.3 FFI 测试覆盖

| 测试文件 | 测试数 | 覆盖范围 |
|---------|--------|---------|
| `ffi_safety.rs` | 17 | 类型拒绝/接受、passport 限制 |
| `ffi_passport_types.rs` | 8 | c_shared/c_borrow、cap、verify_ffi |
| `ffi_verification.rs` | 7 | 合约生成、errno |
| `extern_calls.rs` | 4 | 符号未找到、void 返回 |
| `extern_blocks.rs` | 5 | 解析、多函数、cap 参数 |
| `test-ffi-contracts.sh` | 9 | Z3 验证、运行时合约 |
| **总计** | **50** | |

**未覆盖的关键场景**:
- 无端到端测试调用实际 C 函数并验证返回数据
- 无浮点 ABI 正确性测试
- 无字符串返回所有权测试
- 无回调从 C 调用 Mimi 的集成测试
- 无 `ensures` 后置条件引用 `result` 的测试
- 无 FFI 模糊测试

---

## 十一、统一根因分析

### 语言内部缺口（G1/G2/G5/G10）

G1（闭包）、G5（Shared）、G10（内存安全）共享一个根本原因：**codegen 缺少堆分配句柄的原生支持**。

| 类型 | LLVM 表示 | 缺失部件 |
|------|----------|---------|
| **Shared** | `i8*` 裸指针 (`types.rs:50-53`) | 无 `{refcount, data}` 堆结构 |
| **Closure** | `i64` 裸整数 (`types.rs:93-96`) | 无 `{fn_ptr, env_ptr}` 结构体 |
| **Enum (large)** | `{i32, i64}` 固定结构 | tag 比较逻辑缺失 |

### FFI 层缺口（F1-F8）

FFI 层的核心问题是**ABI 正确性**和**内存契约完整性**：

| 缺口 | 根因 |
|------|------|
| F1 (浮点 ABI) | 调用约定硬编码为 GP 寄存器，未使用 libffi CIF |
| F2 (C 崩溃) | 无信号处理，直接暴露 OS 级错误给用户 |
| F3 (ensures) | 解释器 eval 不支持运行时 scope 注入 |
| F4 (guard 泄漏) | `mem::forget` 用于避免借用检查冲突，但引入泄漏 |
| F5 (类型映射) | FFI 合约系统仅支持标量和指针，无复合类型编组 |
| F6 (内存契约) | C→Mimi 返回值无自动释放机制 |

---

## 十二、路线图

### Phase 1 — FFI 可信基础（P0，3-5 天）

阻塞胶水语言的可用性。完成前不投入其他特性。

| 目标 | 预期工期 | 依赖 |
|------|---------|------|
| G5: Shared 引用计数 | 0.5-1 天 | 无 |
| F4: RwLock guard 泄漏修复 | 0.5 天 | 无 |
| F1: 浮点 ABI 修正 | 1 天 | 无 |
| F3: ensures result 绑定修复 | 0.5 天 | 无 |
| F2: C 崩溃恢复 (SIGSEGV handler) | 1 天 | 无 |
| G10: 编译产物内存释放 | 1-2 天 | 依赖 G5 |

### Phase 2 — 胶水数据交换（P1，2-3 天）

使 Mimi 能够与 C 库交换复杂数据。

| 目标 | 预期工期 | 依赖 |
|------|---------|------|
| G2: 枚举 match tag 比较 | 0.5-1 天 | 无 |
| F5: FFI 类型映射扩展 (List/Record) | 1-2 天 | 无 |
| F6: FFI 内存契约完善 | 0.5 天 | 无 |
| F7: extern ABI 运行时校验 | 0.5 天 | 无 |
| F8: 跨语言回调 | 1 天 | 无 |

### Phase 3 — 工程化（P1-P2，2-3 天）

| 目标 | 预期工期 | 依赖 |
|------|---------|------|
| G9: 跨文件模块 E2E | 1 天 | 无 |
| F9: Python binding generator | 1-2 天 | 无 |
| F10: errno 完整映射 | 0.5 天 | 无 |
| N6: 启用 ASan 测试 | 0.5 天 | 依赖 G10 |
| G1: 闭包 env 结构体 | 1-2 天 | 依赖 G5 |

### Phase 4 — 语言完善（P2-P3）

G3/G4 (测试覆盖)、N1 (ring-buffer)、G6/G8 (arena/async) 等。

---

## 十三、压力集成测试建议

### 13.1 胶水层压力测试

建议新增 `e2e_glue_scenario.mimi`：
1. 定义 `Result<T,E>` 枚举跨 FFI 传递
2. 调用 C 库函数（strlen + 某个接受 double 的函数）
3. 通过 `c_shared` 传递 shared 状态给 C
4. match 枚举解构 C 返回值
5. 编译后运行 + valgrind 检测内存

### 13.2 多特性交叉测试

| 组合 | 风险 | 当前覆盖 |
|------|------|---------|
| 闭包 + shared | 闭包捕获 shared 变量 → env 中引用计数 | 0 |
| enum match + ? | match 解构 Result 后 ? 传播 | 0 |
| shared + spawn | Actor 内部 shared 状态 + await 返回 | 0 |
| FFI + shared + enum | C 函数接收/返回 shared 枚举值 | 0 |

---

## 十四、附录：关键文件索引

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

*本报告基于 2026-06-19 的代码状态。Mimi 定位为编译型胶水语言，FFI 层是产品生命线。如语言版本升级，请同步修订。*
