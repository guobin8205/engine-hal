# engine-hal

> Engine-HAL 概念验证（POC）工作区。
>
> 验证用 Rust 实现的 Godot 兼容运行时，让 Godot 编辑器产出的内容能在 Cocos2d-x 中运行。
>
> 上层文档：[`../docs/research/2026-07-19-engine-hal-research-v2.md`](../docs/research/2026-07-19-engine-hal-research-v2.md)

## POC 总体目标

验证两个独立风险点：

| 阶段 | 目标 | 状态 |
|---|---|---|
| **POC-A** | Rust 能解析 Godot `.tscn` 文件 | ✅ **完成**（17/17 测试通过，实际耗时 ~6.5 小时） |
| **POC-B** | Rust 通过 cxx 调 Cocos2d-x C++ 显示场景 | ⏳ 未开始 |

## 当前进展

### POC-A：纯 Rust 解析（完成 ✅）

详见 [`crates/hal-poc/README.md`](crates/hal-poc/README.md)。

要点：
- 用 pest 写了一份 `.tscn` 文法（~100 行 PEG）
- 支持 Godot 4.x 的所有常用 Variant 类型
- 用真实 fixtures 测试（含中文/emoji/嵌套字典/Packed 数组）
- 提供 `parse_scene()` API + `dump_tscn` 命令行工具

```bash
cd engine-hal
cargo test                  # 17 个测试全过
cargo run --example dump_tscn -- crates/hal-poc/tests/fixtures/sprite.tscn
```

### POC-B：Cocos C++ 集成（待开始）

**前置条件**：POC-A ✅ 已完成。

**计划**：
1. 构建 `E:/repos/cocos/cocos2d-x-3.15.1`（产出 libcocos2d.lib）
2. 搭建最小 Cocos demo（基于 `templates/cpp-template-default`）
3. 用 cxx 桥接 Rust ↔ Cocos C++
4. 端到端：在 Cocos 窗口显示 Godot 编辑的 Sprite

**关键风险**（来自调研）：
- ⚠️ sgs-main 用 v120_xp（VS 2013），Rust 需要 v141+，**ABI 不匹配**
- → POC-B 用 3.15.1 + 现代 MSVC，验证机制可行性
- → 真正接入 sgs-main 需要先把工具集升级到 v141

## 目录结构

```
engine-hal/
├── Cargo.toml                    ← workspace 根
├── README.md                     ← 本文件
├── crates/
│   └── hal-poc/                  ← POC-A 核心 crate
│       ├── README.md             ← POC-A 详细文档
│       ├── src/
│       │   ├── lib.rs
│       │   ├── dump.rs
│       │   ├── parser/
│       │   │   ├── mod.rs
│       │   │   ├── ast.rs
│       │   │   └── tscn.pest
│       │   └── types/
│       │       ├── variant.rs
│       │       └── scene.rs
│       ├── tests/
│       │   ├── parse_basic.rs
│       │   ├── parse_ast.rs
│       │   └── fixtures/
│       └── examples/
│           └── dump_tscn.rs
└── cocos-demo/                   ← POC-B 的 Cocos 工程（待创建）
```

## 设计文档

- [调研报告 v2](../docs/research/2026-07-19-engine-hal-research-v2.md) — 整体方向与业界先例
- [POC 实施计划](../docs/research/) — POC-A/B 详细步骤

## 技术栈

| 依赖 | 版本 | 用途 |
|---|---|---|
| pest | 2.8 | PEG 解析器 |
| pest_derive | 2.8 | derive 宏 |
| serde | 1.0 | 序列化（含 JSON 输出） |
| serde_json | 1.0 | JSON dump |
| glam | 0.33 | Vec2/Vec3/Vec4 数学 |
| cxx | 1.0 | POC-B 的 Rust ↔ C++ FFI |
