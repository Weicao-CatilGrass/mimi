# Mimi 编译器深度审计报告

> **项目**: Mimi v0.7.0 — 基于 MimiSpec 的编译型语言参考实现
> **审计日期**: 2026-06-19（三轮评估整合）
> **代码规模**: Rust 31,639 行（源码）+ 17,770 行（测试）| C 1,277 行（运行时）| Mimi 2,090 行（标准库）
> **依赖**: 14 直接依赖 + 108 总依赖（Cargo.lock）

---

## 一、审计方法论

本报告基于三轮独立代码审计，逐项核实每个风险/缺口的**实际代码状态**（file:line 引用），而非推测。每条发现标注：

- **确认** — 代码证据完全支持
- **部分确认** — 核心问题存在但有缓解措施
- **已修复** — 上轮发现的问题已被代码修改解决
- **误报** — 代码实际行为与报告不符

---

## 二、项目架构概览

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
- **Verifier** (920行) — Z3 SMT 形式化验证
- **LSP** (1,089行) — 语言服务器
- **Formatter/Linter** — 代码格式化与静态分析
- **FFI** (587行) — 跨语言调用边界
- **Diagnostic** — 错误诊断系统

---

## 三、风险总览

### 3.1 风险分布矩阵

```
                    Impact
              Low   Med   High  Critical
Occurrence
  Certain     -     -      -      G5
  Likely      R14   N1     R1     -
  Possible    R17   G1,G2  R8     -
  Rare        G9    G6,G8  G3,G4  -
  By Design   G7    -      -      -
```

### 3.2 风险等级分布

| 等级 | 数量 | 条目 |
|------|------|------|
| **P0 — Critical** | 2 | G5 (Shared 语义分裂), G10 (堆栈内存安全) |
| **P1 — High** | 4 | G1 (闭包 env), G2 (枚举 match), N2 (async 截断), N6 (ASan 禁用) |
| **P2 — Medium** | 6 | G3 (if break/continue), G4 (? 运算符), G6 (arena), G8 (async), N1 (ring-buffer), N3 (无结构化并发) |
| **P3 — Low** | 3 | G7 (借用检查, 设计如此), G9 (跨文件模块), N4 (E2E 框架限制) |
| **已修复** | 9 | R3-R5, R7-R9, R12, R16-R18 等 |
| **Negligible** | 1 | R17 (mimi_try_exit 指针试探) |

---

## 四、P0 — 需立即修复

### G5: Shared 引用计数缺失 — 语义分裂

**严重度**: P0（从 P1 升级）
**位置**: `codegen/func.rs:652-659`, `codegen/block.rs:204-211`, `codegen/actors.rs:450-457`

**现状**:

| 层 | Shared 实现 | clone 行为 | drop 行为 |
|---|------------|-----------|----------|
| **interp** | `Value::Shared(Arc<RwLock<Value>>)` (`interp/value.rs:95`) | `Arc::clone` 增引用计数 (`interp/mod.rs:664`) | Arc drop 减计数，归零释放 |
| **codegen** | `alloca + store` — 与普通 `let` 完全相同 | 独立栈副本，无引用计数 | 栈帧退出直接丢弃 |

**证据链**:
1. `codegen/func.rs:652-659` — `Stmt::SharedLet` 编译为 `alloca + store`，`kind` 字段被 `..` 丢弃
2. `codegen/types.rs:50-53` — `Shared` 映射为 `i8*` 裸指针，无结构体
3. `codegen/builtins/mod.rs:191-198` — `mimi_shared_retain`/`release` 已声明但仅用于 FFI `c_shared`
4. `tests/codegen_e2e.rs:1066-1091` — 测试 `#[ignore]`，注释确认 "Codegen currently treats SharedLet as a plain `let`"

**后果**: 用户无法信任 `shared` 编译后的行为。interp 中 `shared x = y` 创建共享引用，codegen 中创建独立副本。并发程序编译后产生数据竞争。

**修复路径**: 在 SharedLet 时调用 `mimi_shared_retain`，scope exit 时调用 `mimi_shared_release`。运行时函数已存在。

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
| f-string | `expr.rs:2310` | 无 | ❌ 泄漏 |
| string 操作 | `builtins/string.rs` 多处 | 无 | ❌ 泄漏 |

**证据**: `tests/codegen_e2e.rs:1256-1308` — ASan/UBSan/Valgrind 测试全部 `#[ignore]`。`tests/mod.rs:166-248` — 默认 `compile_and_run` 不传 `-fsanitize` 标志。

**后果**: 所有编译后的 Mimi 程序在运行期间持续泄漏内存。长时间运行的服务会 OOM。进程退出由 OS 回收，但运行期间无安全保障。

**修复路径**: 引入编译期引用计数（类似 G5 的 retain/release 模式），或在 arena scope exit 时批量释放。

---

## 五、P1 — 高优先级

### G1: 闭包 env 结构体缺失

**严重度**: P1
**位置**: `codegen/expr.rs:1955-1957`

**现状**: 自由变量收集和参数传递机制已实现（L1867-1883），但返回值是裸函数指针，缺少 `{fn_ptr, env_ptr}` 闭包结构体。

**证据**: `codegen/expr.rs:1955-1957`:
```
// TODO: return closure struct { fn_ptr, captured_env } when runtime supports it
```

**测试覆盖**: interp 有 12 个闭包测试；codegen E2E 测试用普通函数绕过。

---

### G2: 枚举构造器 match 不完整

**严重度**: P1
**位置**: `codegen/expr.rs:965-977`, `codegen/registry.rs:322-331`

**现状**: 枚举类型注册为 `{i32 tag, i64 payload}` 结构体（已实现）。但 match 的 `Constructor` 分支只绑定原始值，不提取 tag 做比较。

**证据**: `codegen/expr.rs:965-977` — `Pattern::Constructor` 处理中 `self.builder.build_store(alloca, scrutinee_iv)` 绑定整个 scrutinee，无 tag 提取逻辑。

**测试覆盖**: E2E 测试用 if/else 替代 match-on-constructors。

---

### N2: async 结果 await 侧截断为 i64

**严重度**: P1
**位置**: `codegen/expr.rs:1511-1522`

**现状**: spawn 侧正确计算 `malloc(result_type.size_of())` 并类型化存储。但 await 侧**始终** cast 为 `i64*` + `build_load(i64)`，截断非 i64 结果。

**证据**: `codegen/expr.rs:1511-1522`:
```rust
let result_typed = self.builder.build_pointer_cast(
    result_ptr, i64_ty.ptr_type(...), "result_i64_ptr")...;
let result_val = self.builder.build_load(
    BasicTypeEnum::IntType(i64_ty), result_typed, "spawn_result_val")...;
```

**后果**: string/list 等复合类型返回值静默损坏。list 的 `{i8*, i64}` struct 只读到 len 字段，data 指针丢失→悬垂指针。

---

### N6: ASan/UBSan 测试全部禁用

**严重度**: P1
**位置**: `tests/codegen_e2e.rs:1012-1308`

**现状**: 9 个内存安全测试全部 `#[ignore]`：
- 2 个 ASan（`e2e_asan_string_ops`, `e2e_asan_list_ops`）
- 3 个 UBSan（`e2e_ubsan_*`）
- 4 个 Valgrind（`e2e_valgrind_*`）

**CI 覆盖**: `run-ci-matrix.sh:95,99` 显式传 `--ignored` 运行，但无自动化 CI 配置（`.github/workflows` 等），需手动执行。

**风险**: 日常开发中默认 `cargo test` 跳过所有内存安全验证。与 G10（裸 malloc 无释放）形成危险组合——已知有内存问题但选择不检测。

---

## 六、P2 — 中优先级

### G3: if 内 break/continue

**严重度**: P2（从 P1 下调）
**位置**: `codegen/block.rs:190` (no-op), `codegen/func.rs:592-613` (handler)

**现状**: `compile_func` 级别的 break/continue 处理器已覆盖常见路径。`compile_block` 的 no-op 仅影响 if-as-expression 场景。IR 测试 3 个通过。

**判定**: 部分确认。常见路径（if-as-statement）已工作。

---

### G4: ? 运算符 E2E 路径

**严重度**: P2（从 P1 下调）
**位置**: `codegen/expr.rs:1535-1619`

**现状**: `compile_try_expr` 完整实现（提取判别式、分支、调用 `mimi_try_exit`）。E2E 测试用 if/else 绕过，无 `?` 直接测试。

**判定**: 部分确认。实现完整但测试覆盖为零。

---

### G6: Arena 分配器降级

**严重度**: P2
**位置**: `codegen/block.rs:217-239,283-318`

**现状**: Arena 编译为 `llvm.stacksave`/`stackrestore`（栈式作用域清理），非堆式 arena。功能等价，提供正确的生命周期保证。

**判定**: 部分确认。不是阻塞性缺口。

---

### G8: async/await pthreads 模拟

**严重度**: P2
**位置**: `codegen/func.rs:15-61`, `codegen/expr.rs:1349-1533`

**现状**: 完整脱糖为 `spawn` + `pthread_create` + `join`。IR 测试 6+ 个通过。限制：仅标量/指针大小结果（见 N2）、无取消机制、无结构化并发。

---

### N1: C 线程池 ring-buffer 溢出

**严重度**: Medium
**位置**: `runtime.c:701`

**现状**: `pool_task_tail++` 无上限检查，超过 1024 任务时覆写未处理的任务。

---

### N3: 无结构化并发

**严重度**: Medium
**位置**: `codegen/expr.rs:1349-1463`

**现状**: spawn 是 fire-and-forget pthread，无取消机制、无错误传播、无父子关系。

---

## 七、P3 — 低优先级 / 设计如此

### G7: 借用检查不在 codegen

**严重度**: P2（设计如此，非 bug）
**位置**: `core/mod.rs:109-273`, `codegen/` 无 borrow 逻辑

**判定**: 确认（by design）。借用检查在 `core/` 类型检查阶段完成，codegen 信任已检查的输入。正确的分层架构。

---

### G9: 跨文件模块 flatten

**严重度**: P3
**位置**: `loader.rs:207-221`, `main.rs:1086-1094`

**现状**: `merge_all()` 将所有模块 flatten 为单一 AST。编译命令支持，E2E 测试框架不支持 `use` 导入。

---

### N4: E2E 测试框架不支持 `use`

**严重度**: Low
**位置**: `tests/mod.rs:1093-1095`

**现状**: `compile_and_run` 无法测试跨文件模块。限制在测试框架，非编译器。

---

### N5: LSP 全量重解析

**严重度**: Low
**位置**: `lsp.rs:146,152`

**现状**: `didChange` 每次全文替换+重解析，大文档性能差。非 bug，影响 UX。

---

## 八、历史风险项状态（已修复/缓解）

以下风险在三轮评估中被发现后已修复或显著缓解：

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

## 九、统一根因分析

G1（闭包）、G5（Shared）、G10（内存安全）共享一个根本原因：**codegen 缺少堆分配句柄的原生支持**。

### 当前状态

| 类型 | LLVM 表示 | 缺失部件 |
|------|----------|---------|
| **Shared** | `i8*` 裸指针 (`types.rs:50-53`) | 无 `{refcount, data}` 堆结构，无 retain/release 调用 |
| **Closure** | `i64` 裸整数 (`types.rs:93-96`) | 无 `{fn_ptr, env_ptr}` 结构体，无 env 打包/解包 |
| **Enum (large)** | `{i32, i64}` 固定结构 (`registry.rs:327-330`) | tag 比较逻辑缺失 (`expr.rs:965-977`) |

### 现有基础设施

代码库中已有部分可复用组件：
- `mimi_shared_retain`/`mimi_shared_release` 已声明 (`builtins/mod.rs:191-198`)
- `DynTrait` 使用 fat pointer `{i8*, i8*}` (`types.rs:114-121`)
- enum struct `{i32 tag, i64 payload}` 已注册 (`registry.rs:327-330`)

### 修复策略

**G5 (Shared) 应先于 G1 (闭包) 修复** — Shared 是引用计数基础，闭包 env 可能捕获 shared 值。

```
Phase 1: Shared 引用计数 (0.5-1 天)
  ├─ SharedLet 时调用 mimi_shared_retain
  ├─ scope exit 时调用 mimi_shared_release
  └─ 更新 codegen/func.rs, block.rs, actors.rs 三个位置

Phase 2: 闭包 env 结构体 (1-2 天)
  ├─ 定义 MimiClosure { fn_ptr, env_ptr, env_size }
  ├─ 注册 LLVM 类型到 registry
  ├─ lambda 编译时打包自由变量到 env struct
  └─ 调用时解包 env 指针作为额外参数

Phase 3: 枚举 match tag 比较 (0.5-1 天)
  ├─ 复用现有 {i32, i64} struct
  ├─ match Constructor 时提取 tag 字段
  └─ 添加 tag 比较分支逻辑
```

---

## 十、压力集成测试建议

当前测试覆盖单特性路径。缺失的交叉场景：

| 组合 | 风险 | 当前覆盖 |
|------|------|---------|
| 闭包 + shared | 闭包捕获 shared 变量 → env 中引用计数 | 0 |
| enum match + ? | match 解构 Result 后 ? 传播 | 0 |
| shared + spawn | Actor 内部 shared 状态 + await 返回 | 0 |
| 闭包 + enum + shared + await | 完整并发场景 | 0 |

建议新增 `e2e_complex_scenario.mimi`：
1. 定义 `Result<T,E>` 枚举
2. 用闭包作为回调传递给 Actor
3. Actor 内部 shared 共享状态
4. match 枚举解构返回值
5. 编译后运行 + valgrind 检测内存

---

## 十一、路线图

| 优先级 | 目标 | 预期工期 | 阻塞关系 |
|--------|------|---------|---------|
| **P0** | G5: Shared 引用计数 | 0.5-1 天 | 无 |
| **P0** | G1: 闭包 env 结构体 | 1-2 天 | 依赖 G5 |
| **P0** | G10: 堆栈内存安全 | 2-3 天 | 依赖 G5 |
| **P1** | G2: 枚举 match tag 比较 | 0.5-1 天 | 无 |
| **P1** | N2: async await 类型化加载 | 0.5 天 | 无 |
| **P1** | N6: 启用 ASan 测试 | 0.5 天 | 依赖 G10 |
| **P2** | G3/G4: 补全测试覆盖 | 0.5 天 | 无 |
| **P2** | N1: ring-buffer 溢出检查 | 0.5 天 | 无 |
| **P3** | G9/N4: 跨文件模块 E2E | 1 天 | 无 |

**建议执行顺序**: G5 → G1 → G2 → G10 → N2 → N6 → 其余

---

## 十二、附录：关键文件索引

| 模块 | 关键文件 | 行数 |
|------|---------|------|
| 解析器 | `src/parser/{mod,parse_expr,parse_stmt,parse_type}.rs` | 2,738 |
| 类型检查 | `src/core/{mod,check_stmt,infer_expr}.rs` | 3,973 |
| 解释器 | `src/interp/{mod,eval,call,builtins,value}.rs` | 5,691 |
| 代码生成 | `src/codegen/{mod,expr,func,block,types,registry}.rs` | ~8,200 |
| FFI | `src/ffi/{runtime,callback}.rs` + `src/interp/ffi_call.rs` | ~1,230 |
| 验证器 | `src/verifier.rs` | 1,153 |
| LSP | `src/lsp.rs` | 1,089 |
| C 运行时 | `src/runtime/mimi_runtime.{c,h}` | 1,277+122 |
| 测试 | `src/tests/` (66 文件) | 17,770 |

---

*本报告基于 2026-06-19 的代码状态。如语言版本升级，请同步修订。*
