//! 场景重建器：调 hal-poc 解析 .tscn，遍历节点调 Cocos facade。
//!
//! B1 阶段只提供最小骨架（API 定义 + 占位），
//! B2 阶段会实现完整的节点遍历和属性映射。

use hal_poc::{parse_scene, SceneData, SceneNode, Variant};

use cxx::let_cxx_string;

use crate::ffi;

/// 解析 .tscn 文件并在 Cocos 窗口里显示。
///
/// 这是 POC-B 的端到端入口（B2 实现）。
/// B1 阶段调用方先用 `create_simple_scene()` 测试 facade 机制。
pub fn build_scene_from_tscn(tscn_path: &str) -> Result<u64, BuildError> {
    let content = std::fs::read_to_string(tscn_path)
        .map_err(|e| BuildError::ReadFile(tscn_path.into(), e.to_string()))?;
    let scene = parse_scene(&content).map_err(|e| BuildError::Parse(e.to_string()))?;
    let scene_handle = build_scene_from_data(&scene);
    ffi::hal_director_run_with_scene(scene_handle);
    Ok(scene_handle)
}

/// 根据解析好的 SceneData 构建到 Cocos。
pub fn build_scene_from_data(scene: &SceneData) -> u64 {
    let cocos_scene = ffi::hal_scene_create();

    // 先建立 "节点名 → u64 句柄" 的映射，处理父子关系
    let mut handles: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    // 第一遍：创建所有节点
    for node in &scene.nodes {
        if let Some(handle) = create_node(node) {
            handles.insert(node.name.clone(), handle);
        }
    }

    // 第二遍：根据 parent 字段建立父子关系
    for node in &scene.nodes {
        if let Some(&child_handle) = handles.get(&node.name) {
            let parent_handle = match &node.parent {
                None => cocos_scene,
                Some(s) if s.as_str() == "." || s.is_empty() => cocos_scene,
                Some(parent_path) => {
                    // parent 可能是 "Path/To" 多级路径，找最后一个 name
                    let parent_name = parent_path.rsplit('/').next().unwrap_or(parent_path);
                    *handles.get(parent_name).unwrap_or(&cocos_scene)
                }
            };
            ffi::hal_node_add_child(parent_handle, child_handle);
        }
    }

    cocos_scene
}

/// 根据节点类型创建对应的 Cocos 节点。返回句柄（失败返回 None）。
fn create_node(node: &SceneNode) -> Option<u64> {
    let node_type = node.r#type.as_deref().unwrap_or("Node");

    let handle = match node_type {
        "Sprite" | "Sprite2D" => create_sprite_node(node)?,
        "Label" => create_label_node(node)?,
        _ => {
            // 其他类型（Control/Panel/Node2D 等）POC-B2 暂不映射，
            // 退化为普通节点（C++ facade 暂用空 Sprite 占位或返回 0）
            0
        }
    };

    if handle != 0 {
        // 应用通用属性
        apply_common_props(handle, node);
    }

    Some(handle)
}

fn create_sprite_node(node: &SceneNode) -> Option<u64> {
    // 从 props 找 texture = ExtResource("id")
    let texture_path = node.props.iter().find_map(|(k, v)| {
        if k == "texture" {
            match v {
                Variant::ExtResource(id) => Some(id.clone()),
                _ => None,
            }
        } else {
            None
        }
    })?;

    // POC-B2 简化：texture 路径直接用 ExtResource id 占位（实际应查 ExtResource 表）
    let_cxx_string!(c_path = texture_path);
    Some(ffi::hal_sprite_create(&c_path))
}

fn create_label_node(node: &SceneNode) -> Option<u64> {
    let text = node
        .props
        .iter()
        .find(|(k, _)| k == "text")
        .and_then(|(_, v)| match v {
            Variant::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let_cxx_string!(c_text = text);
    let_cxx_string!(c_font = "Arial"); // POC 简化：硬编码字体
    Some(ffi::hal_label_create(&c_text, &c_font, 24.0))
}

fn apply_common_props(handle: u64, node: &SceneNode) {
    // position
    if let Some(Variant::Vector2(v)) =
        node.props.iter().find(|(k, _)| k == "position").map(|(_, v)| v)
    {
        ffi::hal_node_set_position(handle, v.x, v.y);
    }

    // visible
    if let Some(Variant::Bool(b)) =
        node.props.iter().find(|(k, _)| k == "visible").map(|(_, v)| v)
    {
        ffi::hal_node_set_visible(handle, *b);
    }

    // modulate (color)
    if let Some(Variant::Color(c)) =
        node.props.iter().find(|(k, _)| k == "modulate").map(|(_, v)| v)
    {
        ffi::hal_node_set_color(handle, ffi::HalColor::from_godot(*c));
    }
}

/// 构建错误。
#[derive(Debug)]
pub enum BuildError {
    ReadFile(String, String),
    Parse(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::ReadFile(path, e) => write!(f, "读取 {} 失败: {}", path, e),
            BuildError::Parse(e) => write!(f, "解析 .tscn 失败: {}", e),
        }
    }
}

impl std::error::Error for BuildError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_error_display() {
        let e = BuildError::Parse("syntax error".into());
        assert!(format!("{}", e).contains("syntax error"));
    }
}
