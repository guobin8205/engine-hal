# POC-B: Rust + cxx + Cocos2d-x C++ 集成

> 验证 engine-hal 方案的另一半核心风险：
> **Rust 能否通过 cxx 安全调用 Cocos2d-x C++ API，并显示 Godot .tscn 场景？**

## 状态

| 阶段 | 状态 | 备注 |
|---|---|---|
| 阶段 0：前置（cocos2d-x 3.17.2 + libcocos2d.lib） | 🔄 进行中 | clone 完成，正在下载依赖 |
| B1：cxx + Ref 机制验证 | ⏳ 未开始 | 最大风险点 |
| B2：端到端场景显示 | ⏳ 未开始 | B1 通过后开始 |

## 引擎与平台

- **引擎**：cocos2d-x 3.17.2（2019，最后一个 3.x）
- **平台**：Windows Win32（先用 x86 走通，x64 后续）
- **Rust 工具链**：`i686-pc-windows-msvc`

## 验证策略：B1 + B2 分阶段

```
POC-B1（机制验证）              POC-B2（端到端）
  最小 C++ facade                扩展 facade 到场景重建
  create_sprite() → u64          build_scene_from_tscn()
  验证 cxx + Ref 安全桥接         验证 Godot 场景在 Cocos 显示
```

## 目录结构

```
poc-b/
├── README.md                  ← 本文件
├── hal-runtime/               ← Rust 侧（cxx bridge）
│   └── src/lib.rs
├── cocos-bridge/              ← C++ facade（薄封装，吸收 Ref/模板/重载）
│   ├── include/hal_bridge.h
│   └── src/hal_bridge.cpp
└── cocos-demo/                ← 最小 Cocos 工程
    ├── Classes/HelloWorldScene.cpp
    └── Resources/test_scene.tscn
```

## 架构设计（基于调研）

### 关键原则

1. **Rust 调 C++ 用 handle 模式**：C++ 持有节点所有权，Rust 只拿 u64 句柄
   - 规避 Cocos `Ref` 引用计数与 Rust 所有权模型的冲突
   - C++ facade 内部管 retain/release
2. **C++ 写薄 facade**：绝不直接暴露 Cocos 原生 API
   - Cocos 用模板（`Vector<T>`）、重载（`addChild` ×3）、`Ref` —— cxx 都不支持
   - facade 全是非模板非重载 C 风格函数
3. **C++ 是宿主**：Cocos 主循环在 C++，通过 `extern "Rust"` 回调 Rust 逻辑

### 桥接接口（B1 最小集）

```rust
// hal-runtime/src/lib.rs
#[cxx::bridge]
mod ffi {
    extern "C++" {
        fn hal_scene_create() -> u64;
        fn hal_sprite_create(texture_path: &str) -> u64;
        fn hal_node_destroy(handle: u64);
        fn hal_node_set_position(handle: u64, x: f32, y: f32);
        fn hal_scene_add_child(scene: u64, child: u64);
        fn hal_director_run_with_scene(scene: u64);
    }
}
```

## 风险与决策点

| 风险 | 应对 |
|---|---|
| cxx + Cocos Ref double-free | handle 模式，C++ 持有所有权 |
| cxx 不支持模板/重载 | facade 全手写 C 风格 |
| 3.17.2 在 VS 2022 编译失败 | 重定目标 v141；最坏换 3.17.1 |
| Cocos 异常跨边界 | facade try/catch 转错误码 |

## 失败决策树

```
B1 失败（cxx + Ref 桥接崩溃）
  → 改用更保守的 handle（C++ 持全部所有权）
  → 最坏：退回方案 B（Rust 输出数据 + Lua 调 Cocos）

B2 失败（场景显示不对）
  → 检查 .tscn 解析 vs Cocos API 映射
  → 通常属性映射问题，逐个修
```
