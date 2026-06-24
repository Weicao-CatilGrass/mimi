# mimisub — Mimi CLI (Mingling Migration)

## 状态

阶段一（试行）进行中。已迁移 1/22 个命令。

## 已建立的基础设施

### 依赖

```toml
mingling = { features = ["dispatch_tree", "extra_macros", "clap", "general_renderer", "parser"] }
clap = "4"
serde = "1"
serde_json = "1"
```

### 文件结构

```
src/
├── main.rs              — program setup, resource 注入, gen_program!()
├── commands/
│   ├── mod.rs           — pub mod stats; 后续加 list, search 等
│   └── stats.rs         — 已迁移的 stats 命令
├── errors/
│   └── mod.rs           — 共享的 Groupped + Serialize 错误类型
└── resources/
    └── mod.rs           — ResCurrentDir(PathBuf), 启动时注入
```

### 建立的设计模式

| 关注点 | 模式 |
|---|---|
| 子命令定义 | `#[dispatcher_clap]` struct，字段即 clap 参数 |
| dispatch_tree | `MOD`+`use MOD::*` 使 gen_program!() 可见 |
| 参数解析 | clap derive，`help = true` 自动生成 help |
| 错误路由 | `route!(expr.map_err(|e| ErrorType { ... }))` |
| 错误类型 | `#[derive(Groupped, Serialize)]` 结构化字段 |
| 渲染器 | 每个类型一个 `#[renderer]` |
| JSON 输出 | `GeneralRendererSetup` + `#[derive(Serialize)]` |
| 共享状态 | `program.with_resource(T)` → `&T` 参数注入 chain/renderer |

### 已验证的命令

```bash
mimisub stats <path>                  # 正常输出
mimisub stats <path> --json          # JSON 输出
mimisub stats --help                 # clap 自动 help
mimisub stats <nonexistent>          # 结构化错误输出
mimisub unknown                      # ErrorDispatcherNotFound
```

## 剩余工作

### 试行阶段继续：list, search 命令迁移

复用相同模式——`#[dispatcher_clap]` + `route!` + renderers。

list/search 的产出是结构化数据，`--json` 立即可用。

### 后续二阶段

- 共享 chain pipeline（resolve_source → read → parse 作为公共步骤）
- Manifest 等更多 Resource
- 补全（`comp` feature）
- 逐步迁移 check/run/build 等复杂命令
