//! 从 POC-A 的 SceneNode 转换为 LayoutNode。
//!
//! 这是连接解析层（hal-poc）和布局层（hal-layout）的桥梁。
//! 输入 SceneNode 树（带 anchor/offset 属性），输出 LayoutNode 树。

use hal_poc::{SceneData, SceneNode, SubResource, Variant};

use crate::anchor::{AnchorsPreset, Offsets};
use crate::layout_tree::{ContainerType, GrowDirection, LayoutNode, Size, SizeFlags};

/// 从 SubResource 的 props 里取 float 值
fn subres_f32(sub: Option<&SubResource>, key: &str) -> Option<f32> {
    let sub = sub?;
    sub.props.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v {
        Variant::Float(f) => Some(*f),
        Variant::Int(i) => Some(*i as f32),
        _ => None,
    })
}

/// 查 SubResource 表，返回 &SubResource
fn lookup_subres<'a>(scene: &'a SceneData, id: &str) -> Option<&'a SubResource> {
    scene.sub_resources.iter().find(|s| s.id == id)
}

/// 从节点的 theme_override_styles/panel 属性查 SubResource 的 content_margin
fn get_panel_margins(scene: &SceneData, node: &SceneNode) -> (f32, f32, f32, f32) {
    // 先从 theme_override_constants 读
    let left = get_f32(node, "theme_override_constants/content_margin_left").unwrap_or(0.0);
    let top = get_f32(node, "theme_override_constants/content_margin_top").unwrap_or(0.0);
    let right = get_f32(node, "theme_override_constants/content_margin_right").unwrap_or(0.0);
    let bottom = get_f32(node, "theme_override_constants/content_margin_bottom").unwrap_or(0.0);
    if left != 0.0 || top != 0.0 || right != 0.0 || bottom != 0.0 {
        return (left, top, right, bottom);
    }
    // 从 SubResource 读
    if let Some(Variant::SubResource(id)) = get_prop(node, "theme_override_styles/panel") {
        let sub = lookup_subres(scene, id);
        let l = subres_f32(sub, "content_margin_left").unwrap_or(0.0);
        let t = subres_f32(sub, "content_margin_top").unwrap_or(0.0);
        let r = subres_f32(sub, "content_margin_right").unwrap_or(0.0);
        let b = subres_f32(sub, "content_margin_bottom").unwrap_or(0.0);
        return (l, t, r, b);
    }
    (0.0, 0.0, 0.0, 0.0)
}

/// 从 SceneData 构建 LayoutNode 树（场景根）。
///
/// 根节点用窗口尺寸布局。
pub fn build_layout_tree(scene: &SceneData, window_size: Size) -> Option<LayoutNode> {
    if scene.nodes.is_empty() {
        return None;
    }

    // 第一个节点作为根（无 parent 或 parent="."）
    let root_scene_node = &scene.nodes[0];
    let mut root = convert_node(scene, root_scene_node);

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

    // 如果当前节点是 TabContainer，只构建 current_tab 对应的子节点。
    // 其余 tab 在 Godot 里 hide（不参与布局），hal 不构建它们。
    let tab_current: Option<i32> = scene.nodes.iter()
        .find(|n| node_path(n, root_name) == current_path)
        .and_then(|n| {
            if n.r#type.as_deref() == Some("TabContainer") {
                Some(get_int(n, "current_tab").unwrap_or(0) as i32)
            } else {
                None
            }
        });

    // visible 的 tab 子节点计数（用于 TabContainer 的 current_tab 匹配）
    let mut visible_child_index = 0i32;

    for node in &scene.nodes {
        // 判断 node 是否是 current_path 的直接子节点
        let is_child = match &node.parent {
            None => false,
            Some(p) if p == "." || p.is_empty() => {
                current_path == root_name
            }
            Some(p) => {
                p == current_path
            }
        };

        if is_child {
            // 跳过 visible=false 的节点（Godot 容器布局中隐藏节点不占空间）
            let is_visible = match node.props.iter().find(|(k, _)| k == "visible").map(|(_, v)| v) {
                Some(Variant::Bool(b)) => *b,
                _ => true,
            };
            if !is_visible {
                continue;
            }
            // TabContainer：跳过非当前 tab
            if let Some(ct) = tab_current {
                if visible_child_index != ct {
                    visible_child_index += 1;
                    continue;
                }
                visible_child_index += 1;
            }
            let mut child = convert_node(scene, node);
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
fn convert_node(scene: &SceneData, scene_node: &SceneNode) -> LayoutNode {
    let (anchors, offsets) = extract_anchors_offsets(scene_node);
    let container = extract_container(scene, scene_node);
    let min_size = extract_min_size(scene_node);

    let mut layout = LayoutNode::new(&scene_node.name, anchors, offsets);
    layout.container = container;
    layout.min_size = min_size;

    layout.size_flags_horizontal = SizeFlags::new(
        get_int(scene_node, "size_flags_horizontal")
            .unwrap_or(default_h_size_flags(scene_node)) as u32,
    );
    layout.size_flags_vertical = SizeFlags::new(
        get_int(scene_node, "size_flags_vertical")
            .unwrap_or(default_v_size_flags(scene_node)) as u32,
    );
    layout.stretch_ratio = get_f32(scene_node, "size_flags_stretch_ratio").unwrap_or(1.0);
    layout.layout_mode = get_int(scene_node, "layout_mode").unwrap_or(0) as i32;
    layout.grow_h = GrowDirection::from_int(get_int(scene_node, "grow_horizontal").unwrap_or(1));
    layout.grow_v = GrowDirection::from_int(get_int(scene_node, "grow_vertical").unwrap_or(1));
    layout.visible = match get_prop(scene_node, "visible") {
        Some(Variant::Bool(b)) => *b,
        _ => true,
    };

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

/// SplitContainer separation 下限：对齐 Godot `_get_separation()` 的 grabber icon 下限。
///
/// Godot 的 _get_separation = MAX(theme_separation, grabber_icon_width)。
/// 默认主题 grabber icon 宽 8px，即使 .tscn 写 separation=0，运行时间距仍是 8。
/// grabber icon 在 headless（有资源）和正常环境都能加载，所以统一用 8 下限。
const SPLIT_GRABBER_WIDTH: f32 = 8.0;

/// 从节点类型推断容器类型。
fn extract_container(scene: &SceneData, node: &SceneNode) -> Option<ContainerType> {
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
        // PanelContainer: 从 StyleBoxFlat SubResource 读 content_margin
        "PanelContainer" => {
            let (left, top, right, bottom) = get_panel_margins(scene, node);
            Some(ContainerType::Margin { left, top, right, bottom })
        }
        // HSplitContainer: split_offset 是相对 default_dragger_position 的偏移
        // separation 直接用 .tscn 的 theme_override 值。
        // 注意：Godot 运行时的 _get_separation = max(theme_sep, grabber_icon_width)，
        // 但 grabber icon 在 headless/无 GL 环境加载失败 → sep = theme_sep。
        // hal-layout 对齐 headless 行为（golden 测试 + Cocos 运行时都无 grabber icon）。
        "HSplitContainer" => {
            let split = get_f32(node, "split_offset")
                .or_else(|| get_int(node, "split_offset").map(|i| i as f32))
                .unwrap_or(0.0);
            let sep = separation.max(SPLIT_GRABBER_WIDTH);
            Some(ContainerType::HSplit { separation: sep, split_offset: split })
        }
        // VSplitContainer: 同 HSplit，轴向垂直
        "VSplitContainer" => {
            let split = get_f32(node, "split_offset")
                .or_else(|| get_int(node, "split_offset").map(|i| i as f32))
                .unwrap_or(0.0);
            let sep = separation.max(SPLIT_GRABBER_WIDTH);
            Some(ContainerType::VSplit { separation: sep, split_offset: split })
        }
        // TabContainer: 只渲染当前 tab（current_tab），顶部留 tab_bar_height。
        // 默认主题 tab_bar 高度约 31px，panel_style margin 默认 0。
        // Godot 的 tab_bar_height = tab_bar.min_size.h + tabbar_style.top + tabbar_style.bottom
        "TabContainer" => {
            let current_tab = get_int(node, "current_tab").unwrap_or(0) as i32;
            Some(ContainerType::Tab {
                tab_bar_height: 31.0,
                current_tab,
                panel_left: 0.0,
                panel_top: 0.0,
                panel_right: 0.0,
                panel_bottom: 0.0,
            })
        }
        // FoldableContainer: 标题栏 + 内容区（移植自 Godot foldable_container.cpp）
        // title_height 来自主题（title_style margin + 字体行高），默认约 31
        // panel margins 来自 panel_style，默认主题 (4,0,4,4)
        // folded=true 时子节点 hide（build_children_recursive 已跳过 visible=false 子节点）
        "FoldableContainer" => {
            let folded = get_prop(node, "folded").and_then(|v| match v {
                Variant::Bool(b) => Some(*b),
                _ => None,
            }).unwrap_or(false);
            let title_position = get_int(node, "title_position").unwrap_or(0) as i32;
            // title_height = title_style_min(8) + max(text_line_h, icon_h=16)
            // panel margins 来自 panel StyleBoxFlat（默认主题 margin 4 四边）
            let title_text = get_string_prop(node, "title").unwrap_or_default();
            let font_size = get_f32(node, "theme_override_font_sizes/font_size").unwrap_or(16.0);
            let text_h = hal_font::line_height(font_size) * title_text.split('\n').count().max(1) as f32;
            let title_height = 8.0 + text_h.max(16.0); // 8 = style top+bottom margin, 16 = fold icon
            Some(ContainerType::Foldable {
                folded,
                title_height,
                title_position,
                panel_left: 4.0,
                panel_top: 4.0,
                panel_right: 4.0,
                panel_bottom: 4.0,
            })
        }
        // AspectRatioContainer: 按宽高比缩放单个子节点，居中对齐
        "AspectRatioContainer" => {
            let ratio = get_f32(node, "ratio").unwrap_or(1.0);
            let stretch_mode = get_int(node, "stretch_mode").unwrap_or(2) as i32;
            let align_h = get_int(node, "alignment_horizontal").unwrap_or(1) as i32;
            let align_v = get_int(node, "alignment_vertical").unwrap_or(1) as i32;
            Some(ContainerType::AspectRatio { ratio, stretch_mode, align_h, align_v })
        }
        // GridContainer: 网格布局，columns 列，子节点按行优先填充
        "GridContainer" => {
            let columns = get_int(node, "columns").unwrap_or(1) as i32;
            let h_sep = get_f32(node, "theme_override_constants/h_separation").unwrap_or(4.0);
            let v_sep = get_f32(node, "theme_override_constants/v_separation").unwrap_or(4.0);
            Some(ContainerType::Grid { columns, h_separation: h_sep, v_separation: v_sep })
        }
        // GraphFrame: GraphElement 子类（Container），布局 = titlebar + 单一内容区。
        // 移植自 Godot graph_frame.cpp::_resort：
        //   内容区 offset = (panel.left, panel.top + titlebar_min.h + titlebar_sb.min.h)
        //   内容区 size = frame.size - panel.margins - (0, titlebar_min.h + titlebar_sb.min.h)
        // 默认主题 panel margins=(18,12,18,12)，titlebar 文字行高 31 + titlebar stylebox 8 = 39。
        // 近似为 Margin{18, 51, 18, 12}（top = panel.top 12 + titlebar_total 39）。
        // GraphNode（叶子，无子节点）会被放进内容区。GraphNode 内部的 slot 布局未实现
        // （需要时再加，sgs-main 业务 UI 不使用 GraphNode）。
        "GraphFrame" => Some(ContainerType::Margin {
            left: 18.0,
            top: 51.0,
            right: 18.0,
            bottom: 12.0,
        }),
        _ => None,
    }
}

/// 节点类型的默认水平 size_flags（对齐 Godot 各 Control 子类的构造函数默认值）。
///
/// .tscn 里只有显式写的 size_flags 才生效（和 anchor 一样）。
/// 没写时用 Godot 类的运行时默认值。大多数 Control 子类默认 SIZE_FILL(1)，
/// 少数在构造函数里覆盖。
fn default_h_size_flags(node: &SceneNode) -> i64 {
    let ty = node.r#type.as_deref().unwrap_or("");
    match ty {
        // VSlider 构造 (slider.h): Slider(VERTICAL) { set_h_size_flags(0); }
        "VSlider" | "VScrollBar" => 0,
        _ => 1, // SIZE_FILL
    }
}

/// 节点类型的默认垂直 size_flags。
///
/// 关键：Label 在构造函数里 `set_v_size_flags(SIZE_SHRINK_CENTER)`，
/// 所以 .tscn 没写 size_flags_vertical 时，Label 的 vsf=4（不是 1）。
/// 这影响 BoxContainer 交叉轴对齐（Label 在 HBox 里垂直居中而非填满）。
fn default_v_size_flags(node: &SceneNode) -> i64 {
    let ty = node.r#type.as_deref().unwrap_or("");
    match ty {
        // Label 构造函数: set_v_size_flags(SIZE_SHRINK_CENTER)
        "Label" | "RichTextLabel" => 4, // SIZE_SHRINK_CENTER
        // HSlider 构造 (slider.h): Slider(HORIZONTAL) { set_v_size_flags(0); }
        // ProgressBar 构造 (progress_bar.cpp): set_v_size_flags(0);
        // → 交叉轴（垂直）不 FILL，用 min_size + 默认对齐（BEGIN）
        "HSlider" | "HScrollBar" | "ProgressBar" => 0,
        _ => 1, // SIZE_FILL
    }
}

/// 提取 minimum_size。
///
/// 优先级：
/// 1. custom_minimum_size（显式设置）
/// 2. 根据节点类型估算（Label 用 text 长度，Button 用 text，其他用默认）
fn extract_min_size(node: &SceneNode) -> Size {
    // 1. 显式 custom_minimum_size
    if let Some(Variant::Vector2(v)) = get_prop(node, "custom_minimum_size") {
        return Size::new(v.x, v.y);
    }

    // 2. 根据类型估算
    let ty = node.r#type.as_deref().unwrap_or("");

    match ty {
        // Label: min_size = 真实字体度量（hal-font 用 Godot 默认 OpenSans SemiBold）
        // min_width = 最长行的像素宽度（fontdue advance width 之和）
        // min_height = font_size × 1.25（行高，含行间距）× 行数
        "Label" => {
            let font_size = get_f32(node, "theme_override_font_sizes/font_size").unwrap_or(16.0);
            let text = get_string_prop(node, "text").unwrap_or_default();
            let text_w = hal_font::text_max_line_width(&text, font_size);
            let line_h = hal_font::line_height(font_size);
            let line_count = text.split('\n').count().max(1);
            Size::new(text_w, line_h * line_count as f32)
        }
        // RichTextLabel: min_size 默认很小（(1,0)），由容器分配尺寸
        // RichTextLabel 的 autofill 行为和 Label 不同，min_size 不基于文字长度
        "RichTextLabel" => Size::new(1.0, 0.0),
        // ColorPickerButton: 色块为主，min 很小（godot 默认主题 (8,8)）
        "ColorPickerButton" => Size::new(8.0, 8.0),
        // Button/CheckBox/CheckButton/LinkButton: 文字宽度（字体度量）+ 主题 padding
        // padding 来自 Godot 默认主题的 StyleBox 内容边距（实测）
        "Button" | "CheckBox" | "CheckButton" | "LinkButton"
        | "MenuButton" | "OptionButton" => {
            let font_size = get_f32(node, "theme_override_font_sizes/font_size").unwrap_or(16.0);
            let text = get_string_prop(node, "text").unwrap_or_default();
            let text_w = hal_font::text_width(&text, font_size);
            // 不同按钮类型的 padding 不同（实测自 Godot 默认主题）：
            //   Button/LinkButton: 文字 + ~10-20px
            //   CheckBox/CheckButton: 复选框图标(~20px) + 文字 + spacing
            //   ColorPickerButton: 色块为主
            //   MenuButton/OptionButton: 含下拉箭头
            let (pad, min_w) = match ty {
                "LinkButton" => (0.0, 0.0),   // 无 StyleBox，min = 纯文字宽度
                "CheckBox" | "CheckButton" => (24.0, 40.0),
                "MenuButton" | "OptionButton" => (30.0, 40.0),
                _ => (16.0, 40.0),  // Button
            };
            let w = (text_w + pad).max(min_w);
            // 高度：LinkButton 无边框，高度 = 文字行高（和 Label 一样，godot 实测 23）
            // 其他按钮（Button/CheckBox 等）有 StyleBox，高度固定 31（godot 默认主题）
            let h = match ty {
                "LinkButton" => hal_font::line_height(font_size),
                _ => 31.0,
            };
            Size::new(w, h)
        }
        // SpinBox: 实际是 LineEdit + 上下箭头，min 宽度由数字位数决定
        "SpinBox" => Size::new(40.0, 31.0),
        // LineEdit: 默认 min 较小（文字 + padding）
        "LineEdit" => Size::new(68.0, 31.0),
        // TextEdit/CodeEdit: 默认 256x200
        "TextEdit" | "CodeEdit" => Size::new(256.0, 200.0),
        // Tree: 默认 100x100
        "Tree" => Size::new(100.0, 100.0),
        // HSeparator/VSeparator: 细长（Godot 默认主题 separation=4 + 上下 margin）
        "HSeparator" => Size::new(0.0, 4.0),
        "VSeparator" => Size::new(4.0, 0.0),
        // Slider/ProgressBar 类：水平方向 min_w 小，高度由主题决定
        // HSlider/HProgressBar 默认主题高度约 16-27
        "HSlider" | "HScrollBar" => Size::new(8.0, 16.0),
        "VSlider" | "VScrollBar" => Size::new(16.0, 8.0),
        "ProgressBar" => Size::new(4.0, 27.0),
        // TextureProgressBar: 无 texture 时 min=(1,1)（不是 128x128）
        // 实际尺寸由 texture 决定，hal-layout 不加载 texture，给最小值
        "TextureProgressBar" => Size::new(1.0, 1.0),
        // 其他 Control: min_size = 0（由子节点决定）
        _ => Size::ZERO,
    }
}

/// 工具：获取字符串属性
fn get_string_prop(node: &SceneNode, key: &str) -> Option<String> {
    match get_prop(node, key) {
        Some(Variant::String(s)) => Some(s.clone()),
        _ => None,
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
                Some(ContainerType::HSplit { .. }) => "HSplit",
                Some(ContainerType::VSplit { .. }) => "VSplit",
                Some(ContainerType::Center) => "Center",
                Some(ContainerType::Tab { .. }) => "Tab",
                Some(ContainerType::Foldable { .. }) => "Foldable",
                Some(ContainerType::AspectRatio { .. }) => "AspectRatio",
                Some(ContainerType::Grid { .. }) => "Grid",
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
