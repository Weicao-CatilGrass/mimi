---
name: 合约验证问题 / Contract Verification Issue
about: 报告 Z3 验证器相关问题 / Report Z3 verifier issues
title: "[Verify] "
labels: verification, z3
assignees: ontonous
---

## 描述 / Description

<!-- 验证器产生了什么意外的结果？/ What unexpected verifier behavior occurred? -->

## Mimi 源代码 / Mimi Source

```mimi
// 完整的可复现源代码 / Complete reproducible source code
```

## 执行命令 / Command Run

```bash
mimi verify source.mimi
# 或
mimi build source.mimi --verify-contracts
```

## 实际输出 / Actual Output

<!-- 请包含 `--stats` 或 `--dump-z3` 的输出 -->

```
```

## 期望输出 / Expected Output

## 额外信息 / Additional Info

- [ ] 验证器产生了误报（假阳性） / False positive
- [ ] 验证器漏报了问题（假阴性） / False negative
- [ ] 验证器崩溃 / Verifier crashed
- [ ] 合约被静默跳过 / Contract silently skipped
