//! 数据模型：从 .tscn 解析出的强类型结构。

mod scene;
mod variant;

pub use scene::{ExtResource, SceneConnection, SceneData, SceneHeader, SceneNode, SubResource};
pub use variant::Variant;
