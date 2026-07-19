//! 场景重建器：调 hal-poc 解析 .tscn，遍历节点调 Cocos facade。
//!
//! B2 完整版：
//! - 解析 ExtResource 表（id → path 映射）
//! - 按节点 type 分发到对应 facade API
//! - 处理父子关系（parent 字段）
//! - 应用常见属性（position/visible/color/text）

use std::collections::HashMap;

use cxx::let_cxx_string;

use hal_poc::{parse_scene, SceneData, SceneNode, Variant};

use crate::ffi;

/// 解析 .tscn 文件并在 Cocos 窗口里显示。
///
/// 这是 POC-B2 的端到端入口。
pub fn build_scene_from_tscn(tscn_path: &str) -> Result<u64, BuildError> {
    eprintln!("POC-B2 [Rust]: 开始加载 {}", tscn_path);

    let content = std::fs::read_to_string(tscn_path)
        .map_err(|e| {
            eprintln!("POC-B2 [Rust]: 读取文件失败: {}", e);
            BuildError::ReadFile(tscn_path.into(), e.to_string())
        })?;

    eprintln!("POC-B2 [Rust]: 文件读取成功，{} 字节", content.len());

    let scene = parse_scene(&content).map_err(|e| {
        eprintln!("POC-B2 [Rust]: 解析失败: {}", e);
        BuildError::Parse(e.to_string())
    })?;

    eprintln!(
        "POC-B2 [Rust]: 解析成功 - {} 节点, {} ext_resources, {} sub_resources",
        scene.nodes.len(),
        scene.ext_resources.len(),
        scene.sub_resources.len()
    );

    let scene_handle = build_scene_from_data(&scene);
    eprintln!(
        "POC-B2 [Rust]: 场景构建完成，handle = {}，调 Director::runWithScene",
        scene_handle
    );
    ffi::hal_director_run_with_scene(scene_handle);
    Ok(scene_handle)
}

/// 根据解析好的 SceneData 构建到 Cocos。
pub fn build_scene_from_data(scene: &SceneData) -> u64 {
    let cocos_scene = ffi::hal_scene_create();

    // 1. 构建 ExtResource 查找表：id → path
    //    Godot 的 ext_resource path 用 res:// 协议，Cocos 需要 relative path
    let ext_resources: HashMap<&str, &str> = scene
        .ext_resources
        .iter()
        .map(|er| (er.id.as_str(), er.path.as_str()))
        .collect();

    // 2. 节点名 → u64 句柄
    let mut handles: HashMap<String, u64> = HashMap::new();

    // 3. 第一遍：创建所有节点
    for node in &scene.nodes {
        if let Some(handle) = create_node(node, &ext_resources) {
            handles.insert(node.name.clone(), handle);
        }
    }

    // 4. 第二遍：根据 parent 字段建立父子关系
    for node in &scene.nodes {
        if let Some(&child_handle) = handles.get(&node.name) {
            let parent_handle = resolve_parent(&node.parent, &handles, cocos_scene);
            ffi::hal_node_add_child(parent_handle, child_handle);
        }
    }

    cocos_scene
}

/// 解析 parent 字段，返回父节点句柄。
/// Godot parent 格式：
///   - None 或 "." 或 "" → 场景根
///   - "PathA" → 同级节点
///   - "PathA/PathB" → 多级路径，取最后一段作为直接父节点名
fn resolve_parent(
    parent: &Option<String>,
    handles: &HashMap<String, u64>,
    scene_root: u64,
) -> u64 {
    match parent {
        None => scene_root,
        Some(s) if s == "." || s.is_empty() => scene_root,
        Some(parent_path) => {
            let parent_name = parent_path.rsplit('/').next().unwrap_or(parent_path);
            *handles.get(parent_name).unwrap_or(&scene_root)
        }
    }
}

/// 根据节点类型创建对应的 Cocos 节点。返回句柄（失败返回 None）。
fn create_node(
    node: &SceneNode,
    ext_resources: &HashMap<&str, &str>,
) -> Option<u64> {
    let node_type = node.r#type.as_deref().unwrap_or("Node");
    eprintln!(
        "POC-B2 [Rust]: 创建节点 '{}' (type={})",
        node.name, node_type
    );

    let handle = match node_type {
        "Sprite" | "Sprite2D" => create_sprite_node(node, ext_resources)?,
        "Label" => create_label_node(node)?,
        // 其他类型（Control/Panel/Node2D 等）POC-B2 暂不映射，跳过
        other => {
            eprintln!("POC-B2 [Rust]: 跳过未支持的节点类型 '{}'", other);
            return None;
        }
    };

    if handle != 0 {
        eprintln!(
            "POC-B2 [Rust]: 节点 '{}' 创建成功，handle={}",
            node.name, handle
        );
        apply_common_props(handle, node);
    } else {
        eprintln!(
            "POC-B2 [Rust]: 节点 '{}' 创建失败（facade 返回 0）",
            node.name
        );
    }

    Some(handle)
}

fn create_sprite_node(
    node: &SceneNode,
    ext_resources: &HashMap<&str, &str>,
) -> Option<u64> {
    // 从 props 找 texture = ExtResource("id")
    let texture_ext_id = node.props.iter().find_map(|(k, v)| {
        if k == "texture" {
            match v {
                Variant::ExtResource(id) => Some(id.clone()),
                _ => None,
            }
        } else {
            None
        }
    })?;

    // 查 ExtResource 表，把 id 转成实际路径
    let texture_path = ext_resources.get(texture_ext_id.as_str()).copied()?;

    // 把 res:// 协议路径转成 Cocos 的相对路径
    // Godot: res://path/to/file.png
    // Cocos: path/to/file.png（相对 Resources 目录）
    let cocos_path = texture_path
        .strip_prefix("res://")
        .unwrap_or(texture_path);

    let_cxx_string!(c_path = cocos_path);
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

    // POC-B2 简化：字体用 Cocos 内置的 Arial 系统字体
    // 实际应该从 props 的 font/theme 读 TTF 路径
    let_cxx_string!(c_text = text);
    let_cxx_string!(c_font = "Arial");
    Some(ffi::hal_label_create(&c_text, &c_font, 24.0))
}

/// Godot 坐标系：左上角原点，Y 向下
/// Cocos 坐标系：左下角原点，Y 向上
/// 转换：cocos_y = WINDOW_HEIGHT - godot_y
///
/// POC 阶段硬编码窗口高度（AppDelegate 用 640）。
/// 正式版应该从 Director 动态获取 VisibleSize。
const WINDOW_HEIGHT: f32 = 640.0;

fn apply_common_props(handle: u64, node: &SceneNode) {
    // position: Vector2(x, y) — 需要翻转 Y 轴（Godot Y 向下，Cocos Y 向上）
    //
    // 注意（Phase 1 待解决）：Label 节点在 Godot 里用 offset_left/top/right/bottom
    // 表达位置，position 是衍生属性。Cocos Label 的 anchor point 默认是 (0.5, 0.5)
    // 中心对齐，和 Godot 的左上角对齐不同，会导致位置整体偏移。
    // 完整的 UI 精确布局需要处理：
    //   - offset_* → position 的精确转换
    //   - anchor point 映射
    //   - 实际窗口尺寸 vs designResolutionSize
    if let Some(Variant::Vector2(v)) =
        node.props.iter().find(|(k, _)| k == "position").map(|(_, v)| v)
    {
        let cocos_y = WINDOW_HEIGHT - v.y;
        ffi::hal_node_set_position(handle, v.x, cocos_y);
    }

    // scale: Vector2(x, y)
    if let Some(Variant::Vector2(v)) =
        node.props.iter().find(|(k, _)| k == "scale").map(|(_, v)| v)
    {
        ffi::hal_node_set_scale(handle, v.x, v.y);
    }

    // visible: bool
    if let Some(Variant::Bool(b)) =
        node.props.iter().find(|(k, _)| k == "visible").map(|(_, v)| v)
    {
        ffi::hal_node_set_visible(handle, *b);
    }

    // modulate: Color(r, g, b, a)
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

    #[test]
    fn resolve_parent_root() {
        let handles = HashMap::new();
        assert_eq!(resolve_parent(&None, &handles, 999), 999);
        assert_eq!(resolve_parent(&Some(".".into()), &handles, 999), 999);
        assert_eq!(resolve_parent(&Some("".into()), &handles, 999), 999);
    }

    #[test]
    fn resolve_parent_named() {
        let mut handles = HashMap::new();
        handles.insert("Parent".into(), 42u64);
        assert_eq!(resolve_parent(&Some("Parent".into()), &handles, 999), 42);
    }

    #[test]
    fn resolve_parent_multilevel() {
        let mut handles = HashMap::new();
        handles.insert("Direct".into(), 77u64);
        // "PathA/Direct" 应该解析到 "Direct"
        assert_eq!(
            resolve_parent(&Some("PathA/Direct".into()), &handles, 999),
            77
        );
    }
}
