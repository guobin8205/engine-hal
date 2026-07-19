//! # hal-layout
//!
//! Engine-HAL UI 布局系统：把 Godot Control 布局翻译/重算为引擎无关的 position+size。
//!
//! ## 两层架构（A + B 并存）
//!
//! ```text
//! 上层 B（重算布局）:
//!   输入 Godot anchor+offset+preset + 容器配置
//!   → Rust 计算每个节点的最终 position+size
//!   → 引擎无关，可测试
//!
//! 下层 A（翻译）:
//!   输入 B 算好的 position+size
//!   → 翻译为 Cocos 调用 setPosition/setContentSize
//!   → 由 scene_builder + facade 实现（POC-B 已验证）
//! ```
//!
//! ## 设计原则
//!
//! - **不依赖外部布局库**（如 taffy）：Godot 的布局算法都在 `control.cpp` 里，
//!   每一行都能找到对应，自己实现更可控
//! - **以 Godot 源码为唯一权威**：所有边界 case 对照 Godot 行为写 golden test
//! - **纯计算 + 引擎无关**：布局结果只是 position+size 的 Vec，可喂给任意后端
//!
//! ## 子模块
//!
//! - `anchor`: 锚点定位（anchor+offset+preset → position+size）
//! - `container`: 容器布局（HBox/VBox/Margin/Center/Grid）
//! - `layout_tree`: 布局树 + dirty-flag 调度

pub mod anchor;
pub mod converter;
pub mod layout_tree;
