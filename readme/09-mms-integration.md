# 09 - MimiSpec 集成

---

## 1. 概述

Mimi 通过 `mms {}` 块支持嵌入 MimiSpec 意图描述，实现**意图→实现**的契约绑定。

| 语言 | 文件后缀 | 核心职责 |
|------|----------|----------|
| MimiSpec | `.mms` | 意图描述、规则约束、AI 协作 |
| Mimi | `.mimi` | 生产实现、内存安全、性能 |

---

## 2. mms 块语法

### 2.1 在函数中嵌入

```mimi
func pay(order: Order, amount: f64) -> Result<(), Err> {
    mms {
        func Pay(order, amount):
            desc "处理支付：检查余额、扣款、改状态"
            rule "支付必须幂等"
            requires: order.status == Pending
            ensures: order.status == Paid
            steps:
                check balance
                if insufficient:
                    error "余额不足" to exit
                charge payment
                on failure:
                    refund
                order.status = Paid to done
    }

    // Mimi 实现
    requires: order.status == Pending
    ensures: order.status == Paid

    let balance = check_balance(order)?;
    if balance < amount {
        return Err("余额不足".into());
    }
    charge_payment(amount)?;
    order.status = OrderStatus::Paid;
    Ok(())
}
```

### 2.2 在类型中嵌入

```mimi
type Order {
    mms {
        type Order:
            desc "订单数据"
            id: u64
            status: OrderStatus
            amount: f64
    }

    id: u64,
    status: OrderStatus,
    amount: f64
}
```

### 2.3 在模块中嵌入

```mimi
module Shop {
    mms {
        module Shop:
            desc "订单管理模块"
            rule "所有操作必须有日志"

            type Order: ...
            func Pay: ...
            func Refund: ...
    }

    // Mimi 实现
}
```

### 2.4 在文件顶层嵌入

```mimi
mms {
    flow OrderLifecycle:
        New to Pending: desc "客户提交"
        Pending:
            to Paid: desc "支付成功"
            to Cancelled: desc "客户取消"
        Paid to Shipped: desc "已发货"
}
```

---

## 3. 设计约束

| 约束 | 规则 | 理由 |
|------|------|------|
| mms 是元数据 | 编译器忽略 mms 块内容 | 保持 Mimi 独立编译能力 |
| 内部是 MimiSpec | 块内使用 MimiSpec 缩进语法 | 保持 MimiSpec 核心价值 |
| 不可嵌套 | mms 内部不能再有 mms | 避免递归复杂度 |
| 位置自由 | 可出现在函数体、类型、模块中 | 灵活性 |
| 契约从 mms 提取 | requires/ensures 从 mms 块提取 | 避免重复表达 |

---

## 4. 契约提取

### 4.1 提取命令

```bash
mimi check --extract-contracts file.mimi
```

### 4.2 输出示例

```
Extracting contracts from file.mimi...

Function: pay
  requires: order.status == Pending
  ensures: order.status == Paid
  rules:
    - "支付必须幂等"

Function: process_order
  requires: order.status == New
  ensures: order.status in [Paid, Cancelled]
```

---

## 5. AI 协作工作流

### 5.1 完整流程

```
1. 意图草图
   人类在 .mms 文件中写意图
   AI 补全 steps/flow/ui
   人类审查，逐步锁定 $/$$

2. 意图嵌入
   人类将 .mms 内容放入 .mimi 文件的 mms {} 块
   或 AI 自动将 .mms 转换为 .mimi + mms {} 块

3. 实现生成
   AI 读取 mms {} 块中的意图
   AI 生成 Mimi 实现代码
   人类审查，锁定 $/$$

4. 契约验证
   编译器提取 mms {} 块中的契约
   编译器验证实现是否满足契约
   输出验证报告

5. 持续协作
   人类修改 mms {} 块中的意图
   AI 更新 Mimi 实现
   编译器重新验证
```

### 5.2 AI 读取意图

AI 可以清晰区分意图和实现：

```mimi
func pay(order: Order, amount: f64) -> Result<(), Err> {
    // AI 读取这些意图：
    mms {
        func Pay(order, amount):
            desc "处理支付：检查余额、扣款、改状态"  // ← AI 知道要做什么
            rule "支付必须幂等"                      // ← AI 知道约束
            requires: order.status == Pending         // ← AI 知道前置条件
            ensures: order.status == Paid             // ← AI 知道后置条件
    }

    // AI 根据意图生成实现
}
```

### 5.3 锁定语义

```mimi
// $$ 锁定：AI 不得修改此函数
func$$ pay(order: Order, amount: f64) -> Result<(), Err> {
    mms {
        func$$ Pay(order, amount):
            desc "处理支付"
            rule "支付必须幂等"
    }

    // 人类确认的实现
}
```

---

## 6. 状态机意图

```mimi
type OrderStatus {
    New,
    Pending,
    Paid,
    Shipped,
    Cancelled
}

mms {
    flow OrderLifecycle:
        New to Pending: desc "客户提交"
        Pending:
            to Paid: desc "支付成功"
            to Cancelled: desc "客户取消"
        Paid to Shipped: desc "已发货"
        Shipped to Delivered: desc "已送达"
}

// Mimi 实现：状态机逻辑
func process_order(order: Order) -> Result<(), string> {
    match order.status {
        OrderStatus::New => {
            order.status = OrderStatus::Pending;
        }
        OrderStatus::Pending => {
            // 支付逻辑
        }
        _ => {}
    }
    Ok(())
}
```

---

## 7. UI 意图

```mimi
type Order {
    id: u64,
    status: OrderStatus,
    amount: f64
}

mms {
    ui OrderPanel binds order:
        stack "订单面板":
            "订单 #order.id" desc "标题"
            parallel "操作栏":
                "支付" desc "按钮" on tap: process_payment(order)
                "取消" desc "按钮" on tap: cancel_order(order)
}

// Mimi 不实现 UI，但 AI 可以从 mms {} 块生成 React/SwiftUI 代码
```

---

## 8. 与现有语法的兼容性

| 语法 | mms 块中 | Mimi 代码中 |
|------|----------|------------|
| `desc "..."` | ✅ | ✅ |
| `rule "..."` | ✅ | ✅ |
| `requires:` / `ensures:` | ✅ | ✅ |
| `math:` | ✅ | ✅ |
| `steps:` | ✅ | - |
| `flow` | ✅ | - |
| `ui` | ✅ | - |
| `$`/`$$`/`?`/`??` | ✅ | ✅ |

---

## 9. 优势

- ✅ MimiSpec 的缩进语法得以保留
- ✅ Mimi 的花括号语法得以保留
- ✅ 两语言通过 mms {} 块自然耦合
- ✅ AI 可以清晰区分意图和实现
- ✅ 编译器可以验证契约满足性
- ✅ 完全向后兼容
