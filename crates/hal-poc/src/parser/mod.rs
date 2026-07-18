//! .tscn 解析器：手写递归下降，忠实移植 Godot `VariantParser`。
//!
//! 子模块：
//! - `stream`: 字符流（对应 Godot `VariantParser::Stream`）
//! - `lexer`: 词法器（对应 Godot `get_token`）
//! - `value`: Variant 值解析（对应 Godot `parse_value`）
//! - `scene_parser`: 顶层场景解析 + tag/assign（对应 Godot `ResourceLoaderText`）

pub mod lexer;
pub mod scene_parser;
pub mod stream;
pub mod value;

pub use scene_parser::parse_scene;
pub use value::ParseError;
