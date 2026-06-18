# CODEGEN 测试框架总结

## 新增测试文件

### 1. `mimi/src/tests/codegen_e2e.rs` — 端到端测试套件 (60个测试)

完整编译链路测试：Mimi源码 → LLVM IR → 目标文件 → 链接 → 执行 → 验证stdout

**测试类别：**
- **基础算术 E2E** (7个): 加减乘除、取模、取反、浮点运算
- **比较运算符 E2E** (6个): ==, !=, <, <=, >, >=
- **逻辑运算符 E2E** (3个): &&, ||, not
- **变量与赋值 E2E** (3个): let绑定、mut赋值、多变量
- **函数 E2E** (6个): 参数传递、多参数、链式调用、递归阶乘、斐波那契
- **控制流 E2E** (7个): if-else、嵌套if、while循环、break、for-range、for-list
- **Match E2E** (3个): 字面量匹配、通配符、变量绑定
- **记录与类型 E2E** (3个): 创建记录、多字段、字段修改
- **列表操作 E2E** (5个): 字面量、索引、map、filter、reduce
- **字符串操作 E2E** (3个): 字面量、f-string插值、多变量插值
- **内置函数 E2E** (5个): range、len、abs、min/max、contains
- **错误处理 E2E** (1个): on_failure基本流程
- **泛型 E2E** (2个): identity函数、wrapper函数
- **引用类型 E2E** (2个): &T, &mut T
- **复杂集成 E2E** (4个): 快速排序、素数、二分搜索

### 2. `mimi/src/tests/codegen_ir.rs` — IR级测试套件 (80个测试)

不执行二进制，仅验证生成的LLVM IR结构

**测试类别：**
- **IR结构测试** (5个): ModuleID、target triple、main函数、entry块、ret指令
- **字面量IR测试** (4个): 整数、浮点、布尔、字符串
- **算术IR测试** (10个): add/sub/mul/div/rem/neg (整数+浮点)
- **比较IR测试** (8个): icmp eq/ne/slt/sle/sgt/sge, fcmp oeq/olt
- **逻辑IR测试** (3个): and/or/xor
- **位运算IR测试** (5个): bitand/bitor/bitxor/shl/shr
- **内存IR测试** (4个): alloca/store/load/gep
- **控制流IR测试** (7个): if分支、条件跳转、无条件跳转、while块、for块、match块
- **函数调用IR测试** (4个): 函数调用、多函数定义、void函数
- **记录IR测试** (3个): struct类型、字段访问、记录创建
- **列表IR测试** (3个): malloc、struct类型、索引
- **外部函数IR测试** (2个): declare、call
- **Actor IR测试** (2个): 构造函数、类型定义
- **Spawn/Await IR测试** (2个): pthread_create、pthread_join
- **Async IR测试** (2个): async body、spawner返回i64
- **泛型IR测试** (2个): 名称修饰、多实例化
- **Cap IR测试** (2个): 编译、函数参数
- **OnFailure IR测试** (3个): 编译、正常退出丢弃、exit保留
- **FString IR测试** (2个): 纯文本、插值
- **内置函数IR测试** (5个): printf、assert块、assert_eq块、range循环
- **引用IR测试** (2个): 指针、store
- **模块IR测试** (2个): 函数、嵌套模块
- **复杂表达式IR测试** (3个): 嵌套算术、链式比较、三元风格if

### 3. `mimi/src/tests/codegen_advanced.rs` — 高级特性测试套件

测试边缘情况和高级语言特性

**测试类别：**
- **幂运算符** (2个): pow调用
- **元组** (2个): 创建、访问
- **列表推导式** (2个): 基本、带guard
- **Try运算符** (1个): ?传播
- **Lambda/闭包** (2个): 表达式、捕获
- **TypeOf/TypeInfo** (2个): 表达式、类型
- **Slice/Range** (2个): 范围、切片
- **Let模式匹配** (2个): 元组、记录
- **SharedLet** (2个): 基本、带类型
- **Unsafe块** (1个): 基本块
- **Alloc块** (1个): arena分配
- **Break带值** (1个): 返回值
- **索引赋值** (1个): arr[i] = x
- **字段赋值** (1个): p.x = v
- **复杂Match** (2个): guard组合、多模式
- **高级泛型** (2个): 函数调用、泛型结构
- **高级Actor** (2个): 方法调用、spawn方法
- **高级Parasteps** (2个): 带spawn、嵌套spawn
- **高级错误处理** (2个): 嵌套on_failure、多语句
- **高级字符串** (2个): 连接、长度
- **高级列表** (2个): append、concat
- **高级记录** (2个): 嵌套、带方法
- **Trait** (2个): 定义、实现
- **模块** (2个): 导入、隐私控制
- **边缘情况** (7个): 空函数、空列表、空字符串、零值、大整数、深嵌套、多局部变量
- **E2E高级测试** (6个): 闭包HOF、模式匹配let、元组解构、泛型identity、列表推导式、字符串操作、高级数学

## 测试框架结构

```
mimi/src/tests/
├── mod.rs                    # 测试模块注册 + compile_and_run() E2E辅助函数
├── codegen_control.rs        # 现有: 153个测试 (控制流、内置函数、类型、Actor等)
├── v1_2_codegen.rs          # 现有: 48个测试 (基础IR编译)
├── codegen_e2e.rs           # 新增: 60个E2E测试
├── codegen_ir.rs            # 新增: 80个IR级测试
└── codegen_advanced.rs      # 新增: 高级特性测试
```

## 辅助函数

所有测试文件共享 `tests/mod.rs` 中的辅助函数：

```rust
// 基础辅助函数
parse(src) -> File                          // 词法分析+解析
run_source(src) -> Value                    // 解析+解释执行
check_source(src) -> Result                 // 类型检查
compile_and_run(src) -> Result<String>      // 完整E2E编译执行流水线

// IR测试辅助函数 (各文件内定义)
compile_to_ir(src) -> String                // 编译为LLVM IR字符串
assert_compiles(src)                        // 验证IR包含"define"
assert_ir_contains(src, pattern)            // 验证IR包含模式
assert_ir_not_contains(src, pattern)        // 验证IR不包含模式
```

## 覆盖范围总结

| 语言特性 | 现有测试 | 新增E2E | 新增IR | 新增高级 | 总计 |
|---------|---------|---------|--------|---------|------|
| 基础算术 | 14 | 7 | 10 | 0 | 31 |
| 比较/逻辑 | 8 | 9 | 11 | 0 | 28 |
| 变量/赋值 | 3 | 3 | 4 | 2 | 12 |
| 函数 | 8 | 6 | 4 | 0 | 18 |
| 控制流 | 8 | 7 | 7 | 0 | 22 |
| Match | 5 | 3 | 0 | 2 | 10 |
| 记录/类型 | 5 | 3 | 3 | 4 | 15 |
| 列表 | 4 | 5 | 3 | 4 | 16 |
| 字符串 | 1 | 3 | 2 | 2 | 8 |
| 内置函数 | 12 | 5 | 5 | 1 | 23 |
| FFI/Extern | 6 | 0 | 2 | 0 | 8 |
| Actor | 3 | 0 | 2 | 2 | 7 |
| Parasteps | 3 | 0 | 0 | 2 | 5 |
| Spawn/Await | 3 | 0 | 2 | 0 | 5 |
| Async | 2 | 0 | 2 | 0 | 4 |
| Cap | 5 | 0 | 2 | 0 | 7 |
| 泛型 | 2 | 2 | 2 | 2 | 8 |
| 错误处理 | 4 | 1 | 3 | 2 | 10 |
| 引用类型 | 2 | 2 | 2 | 0 | 6 |
| 模块 | 2 | 0 | 2 | 2 | 6 |
| 高级特性 | 0 | 0 | 0 | 15 | 15 |
| **总计** | **153** | **60** | **80** | **~40** | **~333** |

## 使用方式

```bash
# 运行所有CODEGEN测试
cargo test codegen_

# 运行特定测试文件
cargo test codegen_e2e
cargo test codegen_ir
cargo test codegen_advanced

# 运行特定测试类别
cargo test e2e_          # 所有E2E测试
cargo test ir_           # 所有IR级测试
cargo test codegen_basic # 基础测试
```

## 注意事项

1. E2E测试需要 `cc` 编译器在PATH中可用
2. 使用 `cc -no-pie` 避免PIE重定位错误
3. 测试生成临时文件并在执行后清理
4. 如果链接器不可用，E2E测试会跳过并打印SKIP消息
