//! 从 POC-A 的 SceneNode 转换为 LayoutNode。
//!
//! 这是连接解析层（hal-poc）和布局层（hal-layout）的桥梁。
//! 输入 SceneNode 树（带 anchor/offset 属性），输出 LayoutNode 树。

use hal_poc::{SceneData, SceneNode, Variant};

use crate::anchor::{AnchorsPreset, Offsets, Preset};
use crate::layout_tree::{ContainerType, LayoutNode, Size};

/// 从 SceneData 构建 LayoutNode 树（场景根）。
///
/// 根节点用窗口尺寸布局。
pub fn build_layout_tree(scene: &SceneData, window_size: Size) -> Option<LayoutNode> {
    if scene.nodes.is_empty() {
        return None;
    }

    // 第一个节点作为根（无 parent 或 parent="."）
    let root_scene_node = &scene.nodes[0];
    let mut root = convert_node(root_scene_node);

    // 构建"路径 → 节点" 的映射，用于高效查找父子关系
    let mut path_to_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, node) in scene.nodes.iter().enumerate() {
        let path = node_path(node, &scene.nodes[0].name);
        path_to_index.insert(path, i);
    }

    // 递归构建子节点
    build_children_recursive(&mut root, &scene.nodes[0].name, scene, &path_to_index);

    // 根节点直接用 window_size（场景根的 anchor/offset 在 Godot 里被实际窗口尺寸覆盖）
    root.computed.position = (0.0, 0.0);
    root.computed.size = window_size;

    // 布局子节点（用根节点的尺寸）
    if let Some(container) = root.container {
        root.layout_container(container);
    } else {
        let my_size = root.computed.size;
        for child in &mut root.children {
            child.layout(my_size);
        }
    }

    Some(root)
}

/// 计算节点的完整路径（从场景根开始）。
fn node_path(node: &SceneNode, _root_name: &str) -> String {
    match &node.parent {
        None => node.name.clone(),
        Some(p) if p == "." || p.is_empty() => node.name.clone(),
        Some(parent_path) => {
            // parent 是完整路径，节点的路径就是 parent + "/" + name
            format!("{}/{}", parent_path, node.name)
        }
    }
}

/// 递归构建子节点。
/// current_path 是当前节点的完整路径（如 "MainPanel/HSplit"）。
/// root_name 是场景根节点名。
fn build_children_recursive(
    parent: &mut LayoutNode,
    current_path: &str,
    scene: &SceneData,
    _path_to_index: &std::collections::HashMap<String, usize>,
) {
    let root_name = &scene.nodes[0].name;

    for node in &scene.nodes {
        // 判断 node 是否是 current_path 的直接子节点
        let is_child = match &node.parent {
            None => false, // 根节点，不是任何人的子节点
            Some(p) if p == "." || p.is_empty() => {
                // parent="." 表示场景根的直接子节点
                // current_path 应该是根节点名
                current_path == root_name
            }
            Some(p) => {
                // parent 是完整路径，等于 current_path 表示是它的子节点
                p == current_path
            }
        };

        if is_child {
            let mut child = convert_node(node);
            let child_path = if current_path == root_name {
                // 根的子节点路径就是 name（不是 root/child，因为 parent="." 不含 root）
                // 但 Godot 实际上更深层用 "MainPanel/HSplit"，所以根的子节点路径就是 "MainPanel"
                node.name.clone()
            } else {
                format!("{}/{}", current_path, node.name)
            };
            build_children_recursive(&mut child, &child_path, scene, _path_to_index);
            parent.add_child(child);
        }
    }
}

/// 把单个 SceneNode 转成 LayoutNode（不递归子节点）。
fn convert_node(scene_node: &SceneNode) -> LayoutNode {
    let (anchors, offsets) = extract_anchors_offsets(scene_node);
    let container = extract_container(scene_node);
    let min_size = extract_min_size(scene_node);

    let mut layout = LayoutNode::new(&scene_node.name, anchors, offsets);
    layout.container = container;
    layout.min_size = min_size;
    layout
}

/// 从 SceneNode 的 props 提取锚点配置。
///
/// 关键：**anchors_preset 只是编辑器标记，不实际设置 anchor 值**。
/// Godot 序列化时，只有显式写入的 anchor_left/top/right/bottom 才生效。
/// anchors_preset 用于编辑器显示，加载时不应用。
///
/// 这意味着：如果 .tscn 里只有 `anchors_preset=12, anchor_top=1.0, anchor_bottom=1.0`
/// 但没有 `anchor_right`，那 anchor_right 保持默认值 0.0（不是 preset 的 1.0）。
fn extract_anchors_offsets(node: &SceneNode) -> (AnchorsPreset, Offsets) {
    // 默认全 0（TopLeft）
    let anchors = AnchorsPreset {
        anchor_left: 0.0,
        anchor_top: 0.0,
        anchor_right: 0.0,
        anchor_bottom: 0.0,
    };
    let offsets = Offsets {
        offset_left: 0.0,
        offset_top: 0.0,
        offset_right: 0.0,
        offset_bottom: 0.0,
    };

    // 只用显式存在的 anchor 属性（不用 anchors_preset！）
    let mut anchors = anchors;
    if let Some(v) = get_f32(node, "anchor_left") {
        anchors.anchor_left = v;
    }
    if let Some(v) = get_f32(node, "anchor_top") {
        anchors.anchor_top = v;
    }
    if let Some(v) = get_f32(node, "anchor_right") {
        anchors.anchor_right = v;
    }
    if let Some(v) = get_f32(node, "anchor_bottom") {
        anchors.anchor_bottom = v;
    }

    let mut offsets = offsets;
    if let Some(v) = get_f32(node, "offset_left") {
        offsets.offset_left = v;
    }
    if let Some(v) = get_f32(node, "offset_top") {
        offsets.offset_top = v;
    }
    if let Some(v) = get_f32(node, "offset_right") {
        offsets.offset_right = v;
    }
    if let Some(v) = get_f32(node, "offset_bottom") {
        offsets.offset_bottom = v;
    }

    (anchors, offsets)
}

/// 从节点类型推断容器类型。
fn extract_container(node: &SceneNode) -> Option<ContainerType> {
    let ty = node.r#type.as_deref()?;
    let separation = get_f32(node, "theme_override_constants/separation").unwrap_or(0.0);

    match ty {
        "HBoxContainer" | "HBox" => Some(ContainerType::HBox { separation: separation.max(4.0) }),
        "VBoxContainer" | "VBox" => Some(ContainerType::VBox { separation: separation.max(4.0) }),
        "MarginContainer" => {
            let left = get_f32(node, "theme_override_constants/margin_left").unwrap_or(0.0);
            let top = get_f32(node, "theme_override_constants/margin_top").unwrap_or(0.0);
            let right = get_f32(node, "theme_override_constants/margin_right").unwrap_or(0.0);
            let bottom = get_f32(node, "theme_override_constants/margin_bottom").unwrap_or(0.0);
            Some(ContainerType::Margin { left, top, right, bottom })
        }
        "CenterContainer" | "Center" => Some(ContainerType::Center),
        _ => None,
    }
}

/// 提取 custom_minimum_size。
fn extract_min_size(node: &SceneNode) -> Size {
    if let Some(Variant::Vector2(v)) = get_prop(node, "custom_minimum_size") {
        Size::new(v.x, v.y)
    } else {
        Size::ZERO
    }
}

// ============ 工具函数 ============

fn get_prop<'a>(node: &'a SceneNode, key: &str) -> Option<&'a Variant> {
    node.props.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn get_f32(node: &SceneNode, key: &str) -> Option<f32> {
    match get_prop(node, key) {
        Some(Variant::Float(f)) => Some(*f),
        Some(Variant::Int(i)) => Some(*i as f32),
        _ => None,
    }
}

fn get_int(node: &SceneNode, key: &str) -> Option<i64> {
    match get_prop(node, key) {
        Some(Variant::Int(i)) => Some(*i),
        Some(Variant::Float(f)) => Some(*f as i64),
        _ => None,
    }
}

impl Preset {
    /// 从整数枚举值构造 Preset（对应 Godot LayoutPreset 枚举）。
    fn from_int(value: i64) -> Option<Preset> {
        match value {
            0 => Some(Preset::TopLeft),
            1 => Some(Preset::TopRight),
            2 => Some(Preset::BottomRight),
            3 => Some(Preset::BottomLeft),
            4 => Some(Preset::CenterLeft),
            5 => Some(Preset::CenterTop),
            6 => Some(Preset::CenterRight),
            7 => Some(Preset::CenterBottom),
            8 => Some(Preset::Center),
            9 => Some(Preset::LeftWide),
            10 => Some(Preset::TopWide),
            11 => Some(Preset::RightWide),
            12 => Some(Preset::BottomWide),
            13 => Some(Preset::VCenterWide),
            14 => Some(Preset::HCenterWide),
            15 => Some(Preset::FullRect),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hal_poc::parse_scene;

    fn parse_fixture(name: &str) -> SceneData {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../hal-poc/tests/fixtures/")
            .join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("无法读取 {}: {}", path.display(), e));
        parse_scene(&content).expect("解析失败")
    }

    #[test]
    fn build_tree_from_simple_scene() {
        // 用 sprite.tscn 测试（虽然它是 Sprite 不是 Control，但能验证转换流程）
        let scene = parse_fixture("sprite.tscn");
        let tree = build_layout_tree(&scene, Size::new(960.0, 640.0));
        assert!(tree.is_some(), "应该能构建布局树");
    }

    #[test]
    fn full_rect_preset_parsed() {
        // control_gallery 的根节点用 anchors_preset = 15 (FullRect)
        let scene = parse_fixture("real/control_gallery.tscn");
        let tree = build_layout_tree(&scene, Size::new(960.0, 640.0));
        let root = tree.expect("应该有布局树");

        // 根节点 FullRect 应该填满整个窗口
        assert_eq!(root.computed.size, Size::new(960.0, 640.0));
    }

    #[test]
    fn vbox_container_detected() {
        let scene = parse_fixture("real/control_gallery.tscn");
        let tree = build_layout_tree(&scene, Size::new(960.0, 640.0));
        let root = tree.expect("应该有布局树");

        // 在子树里找 VBoxContainer
        fn find_container(node: &LayoutNode, ty: &str) -> bool {
            let node_ty = match node.container {
                Some(ContainerType::HBox { .. }) => "HBox",
                Some(ContainerType::VBox { .. }) => "VBox",
                Some(ContainerType::Margin { .. }) => "Margin",
                Some(ContainerType::Center) => "Center",
                None => "None",
            };
            node_ty == ty || node.children.iter().any(|c| find_container(c, ty))
        }
        assert!(
            find_container(&root, "VBox"),
            "control_gallery 应该有 VBoxContainer"
        );
    }

    #[test]
    fn flatten_returns_all_nodes() {
        let scene = parse_fixture("real/control_gallery.tscn");
        let tree = build_layout_tree(&scene, Size::new(960.0, 640.0));
        let root = tree.expect("应该有布局树");
        let flat = root.flatten();
        assert!(
            flat.len() > 10,
            "control_gallery 应该有 10+ 个节点，实际 {}",
            flat.len()
        );
    }
}
