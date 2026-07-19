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
use hal_layout::layout_tree::{FlatNode, Size};
use hal_poc::{parse_scene, SceneData, SceneNode, Variant};

use crate::ffi;

/// 窗口尺寸（和 AppDelegate 的 designResolutionSize 一致）
const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 640.0;

/// 导出文件的固定文件名（写到 exe 工作目录，供 hal-verify 对比工具读取）。
/// expected 由 Rust 侧写入（含 path + handle + Godot/Cocos 双坐标），
/// actual 由 C++ 侧写入（含 handle + Cocos 实际坐标），用 handle 关联。
const EXPECTED_EXPORT_PATH: &str = "cocos_export_expected.json";

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
/// 2. 创建 Cocos 节点（Sprite/Label/ColorRect 占位）
/// 3. 用算好的 position 设置节点位置（翻译层，含 Y 轴翻转）
///
/// 关键：用**节点完整路径**（如 "ControlGallery/MainPanel/HSplitContainer"）做 key，
/// 而不是节点名 —— control_gallery 有大量同名节点（Label/Title/Button），
/// 用 name 会冲突丢节点。
pub fn build_scene_from_data(scene: &SceneData) -> u64 {
    let cocos_scene = ffi::hal_scene_create();
    let root_name = scene.nodes.first().map(|n| n.name.as_str()).unwrap_or("");

    // 0. 用 hal-layout 算布局，按 path 索引（和 FlatNode.path 对齐）
    let window_size = Size::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let layout_tree = build_layout_tree(scene, window_size);
    let mut layout_by_path: HashMap<String, FlatNode> = HashMap::new();
    if let Some(ref tree) = layout_tree {
        for flat in tree.flatten() {
            layout_by_path.insert(flat.path.clone(), flat);
        }
    }

    // 1. 构建 ExtResource 查找表
    let ext_resources: HashMap<&str, &str> = scene
        .ext_resources
        .iter()
        .map(|er| (er.id.as_str(), er.path.as_str()))
        .collect();

    // 2. 创建所有节点，记录 ColorRect 占位符的 handle→path（供导出验证用）。
    // 只导出 ColorRect：Sprite/Label 走真 Cocos 渲染，size 由字体/纹理决定，
    // 和 hal-layout 算的不一致，不参与翻译层对比。
    let mut color_rect_handles: HashMap<u64, String> = HashMap::new();
    let mut ordered: Vec<u64> = Vec::new();

    for node in &scene.nodes {
        let path = scene_node_full_path(node, root_name);
        let layout = layout_by_path.get(&path);
        if let Some(handle) = create_node(node, &path, &ext_resources, layout) {
            if handle != 0 {
                let ty = node.r#type.as_deref().unwrap_or("");
                let is_color_rect = !(ty == "Sprite" || ty == "Sprite2D" || ty == "Label");
                if is_color_rect {
                    color_rect_handles.insert(handle, path);
                }
                ordered.push(handle);
            }
        }
    }

    // 3. 所有节点平铺到 scene 根（全局坐标，避免嵌套双重偏移）
    for handle in &ordered {
        ffi::hal_node_add_child(cocos_scene, *handle);
    }

    // 4. 导出 expected.json（只含 ColorRect 占位符的 path/handle/期望坐标，供 hal-verify 对比）
    export_expected(&color_rect_handles, &layout_by_path);

    cocos_scene
}

/// 构造 SceneNode 的完整路径（和 hal-layout FlatNode.path 对齐，含根名前缀）。
///
/// FlatNode.path 从根名开始累加，如 "ControlGallery/MainPanel/HSplitContainer"。
/// SceneNode.parent 字段对深层节点用相对根的路径（不含根名），需要补上根名前缀。
fn scene_node_full_path(node: &SceneNode, root_name: &str) -> String {
    match &node.parent {
        None => node.name.clone(), // 根节点
        Some(p) if p == "." || p.is_empty() => format!("{}/{}", root_name, node.name),
        Some(p) => format!("{}/{}/{}", root_name, p, node.name),
    }
}

/// 导出 expected.json：每个 ColorRect 节点的 path/handle/Godot 坐标/期望 Cocos 坐标。
///
/// 期望 Cocos 坐标 = Y 轴翻转后的值（LayerColor anchor 在左下角）：
///   cocos_x = godot_x
///   cocos_y = WINDOW_HEIGHT - godot_y - height
///
/// 这个文件和 C++ 侧导出的 actual.json（含 handle + 实际 Cocos 坐标）配合，
/// 用 handle 关联后由 hal-verify 工具对比，验证翻译层 + cxx 桥接是否准确。
fn export_expected(
    color_rect_handles: &HashMap<u64, String>,
    layout_by_path: &HashMap<String, FlatNode>,
) {
    let mut entries: Vec<ExpectedEntry> = Vec::new();
    for (&handle, path) in color_rect_handles {
        let Some(flat) = layout_by_path.get(path) else {
            continue;
        };
        let (gx, gy) = flat.position;
        let (w, h) = (flat.size.width, flat.size.height);
        // 翻译层公式（和 create_control_placeholder 一致）
        let cocos_y = WINDOW_HEIGHT - gy - h;
        entries.push(ExpectedEntry {
            path: path.clone(),
            handle,
            godot_x: gx,
            godot_y: gy,
            w,
            h,
            cocos_x: gx,
            cocos_y,
        });
    }
    // 按 path 排序，输出稳定（方便 diff）
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    match serde_json::to_string_pretty(&entries) {
        Ok(json) => {
            if let Err(e) = std::fs::write(EXPECTED_EXPORT_PATH, json) {
                eprintln!("POC-Export: 写 {} 失败: {}", EXPECTED_EXPORT_PATH, e);
            } else {
                eprintln!(
                    "POC-Export: 写 {} 成功，{} 个节点",
                    EXPECTED_EXPORT_PATH,
                    entries.len()
                );
            }
        }
        Err(e) => eprintln!("POC-Export: 序列化失败: {}", e),
    }
}

/// expected.json 的一条记录（序列化格式和 hal-verify 的反序列化结构对齐）。
#[derive(serde::Serialize)]
struct ExpectedEntry {
    path: String,
    handle: u64,
    godot_x: f32,
    godot_y: f32,
    w: f32,
    h: f32,
    cocos_x: f32,
    cocos_y: f32,
}

/// 根据节点类型创建对应的 Cocos 节点。返回句柄（失败返回 None）。
///
/// `path` 是节点完整路径（含根名，和 FlatNode.path 对齐），用于查 layout。
/// `layout` 是 hal-layout 算出的该节点布局（position+size），None 表示不在布局树里。
fn create_node(
    node: &SceneNode,
    path: &str,
    ext_resources: &HashMap<&str, &str>,
    layout: Option<&FlatNode>,
) -> Option<u64> {
    let node_type = node.r#type.as_deref().unwrap_or("Node");

    let handle = match node_type {
        "Sprite" | "Sprite2D" => create_sprite_node(node, ext_resources)?,
        "Label" => create_label_node(node)?,
        // 所有 Control 类型的节点都用 ColorRect 可视化布局
        _ => create_control_placeholder(node, path, layout)?,
    };

    // Sprite/Label 走 apply_common_props（它们用 Godot position Y 翻转）
    // ColorRect 已经在 create_control_placeholder 里设了正确的 LayerColor 坐标
    if handle != 0 && (node_type == "Sprite" || node_type == "Sprite2D" || node_type == "Label") {
        apply_common_props(handle, node, path, layout);
    }

    Some(handle)
}

/// 为 Control 类型节点创建彩色矩形占位（可视化布局结果）。
/// 注意：ColorRect 直接在这里设 position（不走 apply_common_props），
/// 因为 LayerColor 的坐标系和 Sprite/Label 不同。
fn create_control_placeholder(
    node: &SceneNode,
    _path: &str,
    layout: Option<&FlatNode>,
) -> Option<u64> {
    let flat = layout?;
    let (w, h) = (flat.size.width, flat.size.height);
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
    let (godot_x, godot_y) = flat.position;
    let cocos_y = WINDOW_HEIGHT - godot_y - h;
    ffi::hal_node_set_position(handle, godot_x, cocos_y);

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
    _path: &str,
    layout: Option<&FlatNode>,
) {
    // position：优先用 hal-layout 算出的（Phase 1），fallback 到 props 里的 position
    if let Some(flat) = layout {
        // hal-layout 输出 Godot 坐标系（左上原点，Y 向下），转 Cocos（左下原点，Y 向上）
        let (x, y) = flat.position;
        let cocos_y = WINDOW_HEIGHT - y;
        ffi::hal_node_set_position(handle, x, cocos_y);
    } else if let Some(Variant::Vector2(v)) =
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
    use hal_poc::SceneNode;

    fn mk_node(name: &str, parent: Option<&str>) -> SceneNode {
        SceneNode {
            name: name.into(),
            r#type: None,
            parent: parent.map(|s| s.to_string()),
            index: None,
            instance: None,
            instance_placeholder: None,
            owner: None,
            unique_id: None,
            groups: Vec::new(),
            deferred_node_paths: Vec::new(),
            props: Vec::new(),
        }
    }

    #[test]
    fn build_error_display() {
        let e = BuildError::Parse("syntax error".into());
        assert!(format!("{}", e).contains("syntax error"));
    }

    #[test]
    fn full_path_root() {
        // 根节点：parent=None，path = name
        let n = mk_node("ControlGallery", None);
        assert_eq!(scene_node_full_path(&n, "ControlGallery"), "ControlGallery");
    }

    #[test]
    fn full_path_direct_child() {
        // 根的直接子节点：parent="."
        let n = mk_node("MainPanel", Some("."));
        assert_eq!(scene_node_full_path(&n, "ControlGallery"), "ControlGallery/MainPanel");
    }

    #[test]
    fn full_path_deep() {
        // 深层节点：parent 已是相对根的路径
        let n = mk_node("HSplitContainer", Some("MainPanel"));
        assert_eq!(
            scene_node_full_path(&n, "ControlGallery"),
            "ControlGallery/MainPanel/HSplitContainer"
        );
    }

    #[test]
    fn full_path_deeper() {
        // 更深层：parent 含多级路径
        let n = mk_node("BasicControls", Some("MainPanel/HSplitContainer"));
        assert_eq!(
            scene_node_full_path(&n, "ControlGallery"),
            "ControlGallery/MainPanel/HSplitContainer/BasicControls"
        );
    }
}
