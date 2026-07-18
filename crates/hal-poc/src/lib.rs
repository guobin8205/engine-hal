//! # hal-poc
//!
//! Engine-HAL POC: Rust parser for Godot `.tscn` scene files.
//!
//! **实现方式**：手写递归下降 + 词法器，忠实移植 Godot `VariantParser`
//! （`core/variant/variant_parser.cpp`）。不再使用 pest PEG 文法 ——
//! 因为 PEG 难以表达 Godot 词法层的细节（StringName 是词法 token、
//! `#hex` Color 是词法 token、未知转义=字面量等）。
//!
//! 验证目标（POC-A）：能否准确解析 Godot 4.x 产出的 `.tscn` 文件，
//! 产出强类型的 `SceneData` 结构供下游消费。

pub mod dump;
pub mod parser;
pub mod types;

pub use parser::parse_scene;
pub use types::{
    ExtResource, SceneConnection, SceneData, SceneHeader, SceneNode, SubResource, Variant,
};
