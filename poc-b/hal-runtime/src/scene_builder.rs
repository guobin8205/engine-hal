//! 场景重建器：调 hal-poc 解析 .tscn，遍历节点调 Cocos facade。
//!
//! B2 完整版：
//! - 解析 ExtResource 表（id → path 映射）
//! - 按节点 type 分发到对应 facade API
//! - 处理父子关系（parent 字段）
//! - 应用常见属性（position/visible/color/text）

use std::collections::HashMap;

use cxx::let_cxx_string;

use hal_layout::converter::build_layout_tree;
use hal_layout::layout_tree::Size;
use hal_poc::{parse_scene, SceneData, SceneNode, Variant};

use crate::ffi;

/// 窗口尺寸（和 AppDelegate 的 designResolutionSize 一致）
const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 640.0;

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
///
/// Phase 1 流程：
/// 1. 用 hal-layout 算每个节点的最终 position+size（重算布局）
/// 2. 创建 Cocos 节点（Sprite/Label）
/// 3. 用算好的 position 设置节点位置（翻译层）
pub fn build_scene_from_data(scene: &SceneData) -> u64 {
    let cocos_scene = ffi::hal_scene_create();

    // 0. 用 hal-layout 算布局
    let window_size = Size::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let layout_tree = build_layout_tree(scene, window_size);
    let mut layout_positions: HashMap<String, (f32, f32)> = HashMap::new();
    let mut layout_sizes: HashMap<String, (f32, f32)> = HashMap::new();
    if let Some(ref tree) = layout_tree {
        for flat in tree.flatten() {
            layout_positions.insert(flat.name.clone(), flat.position);
            layout_sizes.insert(flat.name.clone(), (flat.size.width, flat.size.height));
        }
    }

    // 1. 构建 ExtResource 查找表
    let ext_resources: HashMap<&str, &str> = scene
        .ext_resources
        .iter()
        .map(|er| (er.id.as_str(), er.path.as_str()))
        .collect();

    // 2. 节点名 → u64 句柄
    let mut handles: HashMap<String, u64> = HashMap::new();

    // 3. 创建所有节点
    for node in &scene.nodes {
        if let Some(handle) = create_node(node, &ext_resources, &layout_positions, &layout_sizes) {
            handles.insert(node.name.clone(), handle);
        }
    }

    // 4. 根据 parent 字段建立父子关系
    // 注意：ColorRect 的 position 是全局坐标（hal-layout 算的），
    // 所以所有 ColorRect 直接加到 scene 根下（不用嵌套，避免双重偏移）
    // Sprite/Label 保留原来的父子关系（它们的 position 是相对父节点的）
    let mut name_to_handle: HashMap<String, u64> = HashMap::new();
    let mut ordered_handles: Vec<(String, u64, bool)> = Vec::new(); // (name, handle, is_color_rect)
    for node in &scene.nodes {
        if let Some(&h) = handles.get(&node.name) {
            // 只记第一个同名节点（后续同名跳过）
            if !name_to_handle.contains_key(&node.name) {
                name_to_handle.insert(node.name.clone(), h);
                let ty = node.r#type.as_deref().unwrap_or("");
                let is_color_rect = !(ty == "Sprite" || ty == "Sprite2D" || ty == "Label");
                ordered_handles.push((node.name.clone(), h, is_color_rect));
            }
        }
    }

    for (_, handle, is_color_rect) in &ordered_handles {
        if *is_color_rect {
            // ColorRect 直接加到 scene 根（全局坐标）
            ffi::hal_node_add_child(cocos_scene, *handle);
        } else {
            // Sprite/Label 保留父子嵌套
            ffi::hal_node_add_child(cocos_scene, *handle);
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
    layout_map: &HashMap<String, (f32, f32)>,
    layout_sizes: &HashMap<String, (f32, f32)>,
) -> Option<u64> {
    let node_type = node.r#type.as_deref().unwrap_or("Node");

    let handle = match node_type {
        "Sprite" | "Sprite2D" => create_sprite_node(node, ext_resources)?,
        "Label" => create_label_node(node)?,
        // 所有 Control 类型的节点都用 ColorRect 可视化布局
        _ => create_control_placeholder(node, layout_map, layout_sizes)?,
    };

    // Sprite/Label 走 apply_common_props（它们用 Godot position Y 翻转）
    // ColorRect 已经在 create_control_placeholder 里设了正确的 LayerColor 坐标
    if handle != 0 && (node_type == "Sprite" || node_type == "Sprite2D" || node_type == "Label") {
        apply_common_props(handle, node, layout_map);
    }

    Some(handle)
}

/// 为 Control 类型节点创建彩色矩形占位（可视化布局结果）。
/// 注意：ColorRect 直接在这里设 position（不走 apply_common_props），
/// 因为 LayerColor 的坐标系和 Sprite/Label 不同。
fn create_control_placeholder(
    node: &SceneNode,
    layout_positions: &HashMap<String, (f32, f32)>,
    layout_sizes: &HashMap<String, (f32, f32)>,
) -> Option<u64> {
    let &(w, h) = layout_sizes.get(&node.name)?;
    if w < 1.0 || h < 1.0 {
        return None;
    }

    let ty = node.r#type.as_deref().unwrap_or("");
    let color = match ty {
        "VBoxContainer" | "HBoxContainer" | "MarginContainer" | "PanelContainer"
        | "HSplitContainer" | "VSplitContainer" | "TabContainer" | "FoldableContainer"
        | "CenterContainer" => ffi::HalColor::new(0.2, 0.5, 0.9, 0.5), // 不透明蓝
        "Label" | "RichTextLabel" => ffi::HalColor::new(0.2, 0.8, 0.3, 0.5), // 不透明绿
        "Button" | "CheckBox" | "CheckButton" | "LinkButton" | "ColorPickerButton"
        | "SpinBox" | "LineEdit" | "TextEdit" | "CodeEdit" => ffi::HalColor::new(0.9, 0.5, 0.2, 0.5), // 不透明橙
        "ColorRect" => ffi::HalColor::new(0.3, 0.3, 0.3, 0.8), // 深灰（背景）
        "TextureRect" | "TextureProgressBar" => ffi::HalColor::new(0.7, 0.7, 0.2, 0.6), // 黄色
        _ => ffi::HalColor::new(0.5, 0.3, 0.5, 0.3), // 紫灰
    };

    let handle = ffi::hal_color_rect_create(w, h, color);
    if handle == 0 {
        return None;
    }

    // 直接设 position（LayerColor 左下角坐标系）
    // Godot: 左上角原点，Y 向下 → Cocos LayerColor: 左下角原点，Y 向上
    // LayerColor position = 左下角
    // cocos_y = WINDOW_HEIGHT - godot_y - height
    if let Some(&(godot_x, godot_y)) = layout_positions.get(&node.name) {
        let cocos_y = WINDOW_HEIGHT - godot_y - h;
        ffi::hal_node_set_position(handle, godot_x, cocos_y);
        eprintln!(
            "POC-ColorRect: {} type={} godot=({:.0},{:.0}) size=({:.0},{:.0}) → Cocos ({:.0},{:.0})",
            node.name, ty, godot_x, godot_y, w, h, godot_x, cocos_y
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

fn apply_common_props(
    handle: u64,
    node: &SceneNode,
    layout_map: &HashMap<String, (f32, f32)>,
) {
    // position：优先用 hal-layout 算出的（Phase 1），fallback 到 props 里的 position
    if let Some(&(x, y)) = layout_map.get(&node.name) {
        // hal-layout 输出 Godot 坐标系（左上原点，Y 向下），转 Cocos（左下原点，Y 向上）
        let cocos_y = WINDOW_HEIGHT - y;
        ffi::hal_node_set_position(handle, x, cocos_y);
        eprintln!(
            "POC-Pos[layout]: {} → Cocos ({:.0}, {:.0})",
            node.name, x, cocos_y
        );
    } else if let Some(Variant::Vector2(v)) =
        node.props.iter().find(|(k, _)| k == "position").map(|(_, v)| v)
    {
        let cocos_y = WINDOW_HEIGHT - v.y;
        ffi::hal_node_set_position(handle, v.x, cocos_y);
        eprintln!(
            "POC-Pos[props]: {} → Cocos ({:.0}, {:.0})",
            node.name, v.x, cocos_y
        );
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
